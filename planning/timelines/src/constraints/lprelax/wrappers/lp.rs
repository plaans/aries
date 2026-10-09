use std::collections::HashMap;

use itertools::Itertools;

use aries_solver::backtrack::{Backtrack, DecLvl};
use aries_solver::core::state::{Domains, DomainsSnapshot, Explanation, InferenceCause};
use aries_solver::core::{IntCst, Lit, Var, views::Term};
use aries_solver::prelude::Conjunction;
use aries_solver::reasoners::{Contradiction, ReasonerId, Theory};

use aries_solver::reasoners::lp::{BoundRestriction, Lp, LpSum, LpVar};

use crate::lprelax::encoder::problem::{ColTag, LpRelaxProblem, RowExprType};
use crate::lprelax::{ARIES_LPRELAX_CHECKS, ARIES_LPRELAX_MERGE_EQUAL_COLUMNS, LpRelaxChecks, LpRelaxEncoder};
use crate::{IntTerm, SchedEncoder};

/// Fractions of the presence literals of the relaxed transitions and sources that trigger a new solve of the LP, when that many are decided
/// (only once, until re-armed: see [`PHASE_REARM_MARGIN`]).
const PHASES: [f64; 3] = [0.5, 0.75, 0.9];

/// A phase is re-armed (i.e. can trigger a solve again) once the fraction of decided presence literals falls this much below it, e.g. after backtracking.
/// Not less, to avoid many solves if the search "oscillates" around a phase.
const PHASE_REARM_MARGIN: f64 = 0.1;

/// Wrapper over the incremental LP reasoner, specifically for the LP relaxation problem.
///
/// For efficiency, the LP relaxation problem is posted to the reasoner after two steps:
/// - 1st, at the root level (but after the initial propagation), the [`LpRelaxEncoder`] is built,
///   together with a copy / cache of the domains at that level.
/// - 2nd, after all assumptions have been propagated, the [`LpRelaxEncoder`] builds the LP relaxation problem,
///   and simplifies it, notably using the cached propagated domains from the root level (*NOT* the domains after the propagation of assumptions).
///   It then binds the reasoner (the LP's columns) to the main model (events on its literals).
///
/// This 2-stage approach allows to build the encoder more efficiently (as it will be done after the first propagation)
/// and to avoid building and solving the LP if it can be detected as unsatisfiable without it, after all assumptions are propagated.
///
/// The LP is solved once when posted, then depending on [`ARIES_LPRELAX_CHECKS`]: never again,
/// once each time a new phase of decided presence literals is reached (see [`PHASES`]),
/// or at every quiescent propagation where the fixed columns changed since the last solve.
#[derive(Clone)]
pub(crate) struct LpRelaxIncr {
    lp: Lp,
    ctx: SchedEncoder,

    /// Number of assumption levels to wait for before building the relaxed problem.
    num_assumptions: usize,

    /// The encoder of stage 1, with the domains it was built against. `None` until then.
    encoder: Option<(LpRelaxEncoder, Domains)>,
    /// Whether stage 2 has run.
    posted: bool,

    num_events: u32,
    propagation_calls: usize,

    /// The (non-constant) presence literals of the relaxed transitions and sources, whose decided fraction defines the phases
    presence_lits: Vec<Lit>,
    /// Index of the current phase (see [`PHASES`])
    current_phase: usize,
    /// The LP's number of changes in fixed columns at its last solve (see [`Lp::num_fixed_changes`])
    fixed_changes_at_last_check: usize,
}

impl LpRelaxIncr {
    pub(crate) fn new(ctx: SchedEncoder, num_assumptions: usize) -> Self {
        let mut lp = Lp::default();
        lp.activate();

        Self {
            lp,
            ctx,
            num_assumptions,
            encoder: None,
            posted: false,
            num_events: 0,
            propagation_calls: 0,
            presence_lits: Vec::new(),
            current_phase: 0,
            fixed_changes_at_last_check: 0,
        }
    }

    /// Stage 1: collect the transitions and their supports. Must be done at the root level.
    fn build_encoder(&mut self, doms: &Domains) {
        debug_assert!(self.encoder.is_none() && !self.posted);
        let time = std::time::Instant::now();

        let encoder = LpRelaxEncoder::with_transitions_from(&self.ctx);

        lprelax_log!(
            "Built LpRelaxEncoder after {} propagation calls (decision level {:?}, num events: {}) in {}",
            self.propagation_calls,
            doms.current_decision_level(),
            doms.num_events(),
            time.elapsed().as_secs_f64(),
        );

        self.encoder = Some((encoder, doms.clone()));
    }

    /// Stage 2: encode the problem, simplify it, and hand both it and its bindings to the reasoner.
    fn post_relaxation(&mut self, doms: &Domains) {
        debug_assert!(!self.posted);
        let Some((encoder, base_doms)) = self.encoder.as_mut() else {
            unreachable!("stage 2 only runs once stage 1 has")
        };
        let time = std::time::Instant::now();
        let base_decision_level = base_doms.current_decision_level();

        let mut problem = encoder.encode(&self.ctx);
        problem.simplify(encoder, &self.ctx, base_doms, ARIES_LPRELAX_MERGE_EQUAL_COLUMNS.get());

        let vars = post_variables_and_rows(&problem, base_doms, &mut self.lp);
        self.presence_lits = post_bindings(&vars, encoder, &self.ctx, base_doms, &mut self.lp);

        self.posted = true;

        lprelax_log!(
            "Posted LpRelaxProblem ({} variables, {} rows) after {} propagation calls (decision level {:?}, num events: {}) from the model at level {:?} in {}",
            problem.cols().unwrap().len(),
            problem.rows().len(),
            self.propagation_calls,
            doms.current_decision_level(),
            doms.num_events(),
            base_decision_level,
            time.elapsed().as_secs_f64(),
        );
    }
}

impl Theory for LpRelaxIncr {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Extra(42)
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        self.propagation_calls += 1;

        // Quiescence: the reasoners preceding this one (in propagation order) have inferred all they could.
        let quiescent = {
            let num_events = model.trail().num_events();
            let quiescent = num_events == self.num_events;
            self.num_events = num_events;
            quiescent
        };

        let mut check_feas = false;

        if quiescent {
            if self.encoder.is_none() && model.current_decision_level() == DecLvl::ROOT {
                self.build_encoder(model);
            }
            if self.encoder.is_some()
                && !self.posted
                && self.current_decision_level().to_int() as usize >= self.num_assumptions
            {
                self.post_relaxation(model);
                check_feas = true;
            }
        }

        if !self.posted {
            return Ok(());
        }

        // Sync the LP's bounds with the model, without solving it.
        self.lp.sync_bounds(model)?;

        if quiescent && ARIES_LPRELAX_CHECKS.get() == LpRelaxChecks::Phases {
            let num_total = self.presence_lits.len();
            let num_decided = self
                .presence_lits
                .iter()
                .filter(|&&lit| model.entails(lit) || model.entails(!lit))
                .count();
            let decided_ratio = num_decided as f64 / num_total.max(1) as f64;
            while self.current_phase > 0 && decided_ratio < PHASES[self.current_phase - 1] - PHASE_REARM_MARGIN {
                self.current_phase -= 1;
            }
            let num_phase_reached = PHASES.iter().take_while(|&&phase| decided_ratio >= phase).count();
            if num_phase_reached > self.current_phase {
                self.current_phase = num_phase_reached;
                check_feas = true;
                lprelax_log!(
                    "Reached the phase of {}% decided presences ({} / {}) at decision level {:?}",
                    100. * PHASES[num_phase_reached - 1],
                    num_decided,
                    num_total,
                    model.current_decision_level(),
                );
            }
        }

        if quiescent
            && ARIES_LPRELAX_CHECKS.get() == LpRelaxChecks::Changes
            && self.lp.num_fixed_changes() != self.fixed_changes_at_last_check
        {
            check_feas = true;
        }

        if !check_feas {
            return Ok(());
        }

        self.fixed_changes_at_last_check = self.lp.num_fixed_changes();
        self.lp.check_feasibility(model)
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        self.lp.explain(literal, context, model, out_explanation);
    }

    fn print_stats(&self) {
        self.lp.print_stats();
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for LpRelaxIncr {
    fn save_state(&mut self) -> DecLvl {
        self.lp.save_state()
    }
    fn num_saved(&self) -> u32 {
        self.lp.num_saved()
    }
    fn restore_last(&mut self) {
        self.lp.restore_last();
    }
}

/// Adds one `[0, 1]` column per column of the problem, then one row per row, and returns the mapping from column tags to LP columns.
///
/// With column merging enabled, several tags share a column, and all of them are in the mapping
/// (see [`LpRelaxProblem::col_index`]), so that each of their bindings constrains that one column.
fn post_variables_and_rows(problem: &LpRelaxProblem, doms: &Domains, lp: &mut Lp) -> HashMap<ColTag, LpVar> {
    let problem_cols = problem
        .cols()
        .expect("the problem must be simplified or sealed before being posted");

    let lp_vars = problem_cols
        .iter()
        .map(|_| lp.create_auxiliary_variable(0, 1))
        .collect::<Vec<_>>();
    let mut vars: HashMap<ColTag, LpVar> = problem
        .col_index()
        .unwrap()
        .iter()
        .map(|(&tag, &i)| (tag, lp_vars[i]))
        .collect();

    // A tag whose value the simplification knows doesn't correspond to a column anymore,
    // but a binding on it may still contradict that value.
    // The fix is for the bindings to go to a column fixed to that value (one per value, outside of any row).
    let fixed = [lp.create_auxiliary_variable(0, 0), lp.create_auxiliary_variable(1, 1)];
    vars.extend(
        problem
            .col_known_values()
            .map(|(tag, value)| (tag, fixed[value as usize])),
    );

    debug_assert!(
        problem.rows().iter().all(|row| {
            row.lhs()
                .iter()
                .chain(row.rhs())
                .map(|&(_, tag)| vars[&tag])
                .all_unique()
        }),
        "a row uses the same LP variable twice: its coefficients should have been summed",
    );

    for row in problem.rows() {
        let sum = LpSum::new(
            row.lhs()
                .iter()
                .map(|&(coef, tag)| (vars[&tag], coef))
                .chain(row.rhs().iter().map(|&(coef, tag)| (vars[&tag], -coef))),
        );

        match row.tpe {
            // `sum <= cst`
            RowExprType::Leq => lp.add_linear_leq_constraint(sum, -row.cst(), Lit::TRUE, doms),
            // `sum >= cst`, i.e. `-sum + cst <= 0`
            RowExprType::Geq => lp.add_linear_leq_constraint(negated(&sum), row.cst(), Lit::TRUE, doms),
            RowExprType::Eq => {
                lp.add_linear_leq_constraint(negated(&sum), row.cst(), Lit::TRUE, doms);
                lp.add_linear_leq_constraint(sum, -row.cst(), Lit::TRUE, doms);
            }
        }
    }

    vars
}

fn negated(sum: &LpSum) -> LpSum {
    LpSum::new(sum.iter().map(|(var, coef)| (var, -coef)))
}

/// Ties the lifted columns (as well as term grounding columns) of the LP to the literals of the main model that decide them.
///
/// Every binding is a half-binding: it constrains the column when the main model's literal becomes
/// entailed, and never the other way round.
///
/// Returns the (non-constant) presence literals of the relaxed transitions and sources.
fn post_bindings(
    vars: &HashMap<ColTag, LpVar>,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    lp: &mut Lp,
) -> Vec<Lit> {
    // Bind lifted presence variables of the LP with corresponding literals in the main CSP.

    let presence_lits_and_vars = {
        let mut res = HashMap::<Lit, Vec<LpVar>>::new();

        for (source, transitions) in encoder.iter_sources() {
            for &trans_id in transitions {
                let Some(&var) = vars.get(&ColTag::PresenceTransition(trans_id, None)) else {
                    continue;
                };
                res.entry(encoder.transitions.get_prez(trans_id, ctx))
                    .or_default()
                    .push(var);
            }
            let Some(&var) = vars.get(&ColTag::PresenceSource(source, None)) else {
                continue;
            };
            res.entry(encoder.get_source_prez(source, ctx)).or_default().push(var);
        }
        res
    };

    let presence_lits = presence_lits_and_vars
        .keys()
        .filter(|lit| !lit.tautological() && !lit.absurd())
        .copied()
        .collect();

    for (lit, lit_vars) in presence_lits_and_vars {
        for &var in &lit_vars {
            if lit.tautological() {
                lp.add_bound_update_trigger(Conjunction::tautology(), BoundRestriction::geq(var, 1), doms);
            } else if lit.absurd() {
                lp.add_bound_update_trigger(Conjunction::tautology(), BoundRestriction::leq(var, 0), doms);
            } else {
                let p = lit.variable();
                debug_assert!(p != Var::ZERO && lit == p.geq(1));

                lp.add_bound_update_trigger([doms.presence(p), lit].into(), BoundRestriction::geq(var, 1), doms);
                lp.add_bound_update_trigger((!lit).into(), BoundRestriction::leq(var, 0), doms);
            }
        }
    }

    // Bind term grounding variables of the LP with corresponding literals in the main CSP.

    for (term, value) in encoder.iter_sorted_all_only_assignments() {
        debug_assert!(!term.is_cst());
        let Some(&col_var) = vars.get(&ColTag::TermGround(term, value)) else {
            continue;
        };
        let var = term.variable();
        assert!(var != Var::ZERO);

        let Some(x) = var_value_of(term, value) else {
            // There exists no `x` value that `var` could ever take such that `term = value`
            lp.add_bound_update_trigger(Conjunction::tautology(), BoundRestriction::leq(col_var, 0), doms);
            continue;
        };

        // The column is pinned to 0 as soon as we're sure the variable cannot take the value `x`
        if let Some(above) = x.checked_add(1) {
            lp.add_bound_update_trigger(var.geq(above).into(), BoundRestriction::leq(col_var, 0), doms);
        }
        if let Some(below) = x.checked_sub(1) {
            lp.add_bound_update_trigger(var.leq(below).into(), BoundRestriction::leq(col_var, 0), doms);
        }
    }

    // Bind lifted support variables of the LP with corresponding literals in the main CSP.

    for &((out_trans_id, in_trans_id), active) in encoder.supports.unsorted_out() {
        let Some(lit) = active else { continue };
        let Some(&var) = vars.get(&ColTag::Support(out_trans_id, in_trans_id, None)) else {
            continue;
        };
        debug_assert!(lit.variable() != Var::ZERO && lit == lit.variable().geq(1));

        lp.add_bound_update_trigger((!lit).into(), BoundRestriction::leq(var, 0), doms);

        // NOTE: the converse half-binding, raising the column's lower bound to 1 on an *active* link, is deliberately absent by default !!
        // Indeed, it is *unsound* if condition transitions are allowed to support other transitions (i.e. act as out-transitions).
        // See [`ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS`].
        // This is because we bound the "out-flow" of a support by 1, but the main model lets one effect support several conditions,
        // so forcing two of those columns to 1 would make the LP relaxation report a contradiction that the main model does not have.
        //
        // // lp.add_bound_update_trigger([doms.presence(lit), lit], BoundConstraint::geq(col_var, 1), doms);
    }

    presence_lits
}

/// The value `x` of a term's variable that makes the term `a*x + b` equal to `value`, if any.
///
/// A grounding records values of the *term*, while the literals of the main model are on its variable, hence `x = (value - b) / a`.
/// But most of the time `a` is 1 and `b` is 0.
///
/// `None` means strictly that no integer `x` qualifies, and an arithmetic overflow panics instead of returning `None`.
/// Indeed, if an arithmetic overflow returned `None`, we could unsoundly pin a column to 0.
fn var_value_of(term: IntTerm, value: IntCst) -> Option<IntCst> {
    const OVERFLOW_MSG: &str = "overflow while computing the variable value of a term grounding";

    let factor = term.scaled_var.factor;
    debug_assert!(factor != 0, "a non-constant term shouldn't have a 0 factor");

    // The checked rem / div can only trip on `INT_CST_MIN` with `factor == -1`,
    // as division by zero is excluded by the assert above.
    let num = value.checked_sub(term.constant).expect(OVERFLOW_MSG);
    (num.checked_rem(factor).expect(OVERFLOW_MSG) == 0).then(|| num.checked_div(factor).expect(OVERFLOW_MSG))
}
