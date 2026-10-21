use std::collections::HashMap;

use itertools::Itertools;

use aries_linexpr::ComparisonOp;
use aries_solver::backtrack::{Backtrack, DecLvl};
use aries_solver::core::state::{Domains, DomainsSnapshot, Explanation, InferenceCause};
use aries_solver::core::{INT_CST_MAX, IntCst, Lit, Var, views::Term};
use aries_solver::reasoners::{Contradiction, ReasonerId, Theory};

use aries_solver::reasoners::lp::{BoundRestriction, Lp, LpOptions, LpSum, LpVar};

use crate::lprelax::ARIES_LPRELAX_MERGE_EQUAL_COLUMNS;
use crate::lprelax::encoder::LpRelaxEncoder;
use crate::lprelax::encoder::problem::{ColTag, LpRelaxProblem};
use crate::{IntTerm, SchedEncoder};

/// Wrapper over the incremental LP reasoner, specifically for the LP relaxation problem.
///
/// For efficiency, the LP relaxation problem is posted to the reasoner after two steps:
/// - 1st, at the root level (but after the initial propagation),
///   the [`LpRelaxEncoder`] is built, together with a copy / cache of the domains at that level.
/// - 2nd, once the search starts (i.e. after all assumptions have been propagated),
///   the [`LpRelaxEncoder`] builds the LP relaxation problem,
///   and simplifies it, notably using the cached propagated domains from the root level (*NOT* the domains after the propagation of assumptions).
///   It then binds the reasoner (the LP's columns) to the main model (events on its literals).
///
/// This 2-stage approach allows to build the encoder more efficiently (as it will be done after the first propagation)
/// and to avoid building and solving the LP if the main model can be proven unsatisfiable before search without it
///
/// Once posted, the LP is updated and incrementally checked at every propagation by the reasoner itself,
/// with limited effort (see the pivot budget in [`LpOptions`]).
#[derive(Clone)]
pub(crate) struct LpRelaxIncr {
    lp: Lp,
    ctx: SchedEncoder,

    /// The encoder of stage 1, with the domains it was built against. `None` until then.
    encoder: Option<(LpRelaxEncoder, Domains)>,
    /// Whether the search has started, i.e. all assumptions are posted. The relaxed problem is only built afterwards.
    search_started: bool,
    /// Whether stage 2 has run.
    lp_posted: bool,

    num_events: u32,
}

impl LpRelaxIncr {
    pub(crate) fn new(ctx: SchedEncoder, lp_options: LpOptions) -> Self {
        let mut lp = Lp::new(lp_options);
        lp.activate();

        Self {
            lp,
            ctx,
            encoder: None,
            search_started: false,
            lp_posted: false,
            num_events: 0,
        }
    }

    /// Stage 1: collect the transitions and their supports. Must be done at the root level.
    fn build_encoder(&mut self, doms: &Domains) {
        debug_assert!(self.encoder.is_none() && !self.lp_posted);

        let time = std::time::Instant::now();

        let encoder = LpRelaxEncoder::with_transitions_from(&self.ctx);

        lprelax_log!(
            "Built LpRelaxEncoder at decision level {:?} (num events: {}) in {}",
            doms.current_decision_level(),
            doms.num_events(),
            time.elapsed().as_secs_f64(),
        );

        self.encoder = Some((encoder, doms.clone()));
    }

    /// Stage 2: ground and encode the problem, simplify it, and hand both it and its bindings to the reasoner.
    fn post_relaxation(&mut self, doms: &Domains) {
        debug_assert!(!self.lp_posted);

        let Some((encoder, base_doms)) = self.encoder.as_mut() else {
            unreachable!("stage 2 only runs once stage 1 has")
        };
        let time = std::time::Instant::now();
        let base_decision_level = base_doms.current_decision_level();

        let mut problem = encoder.encode(&self.ctx);
        problem.seal_simplified(encoder, &self.ctx, base_doms, ARIES_LPRELAX_MERGE_EQUAL_COLUMNS.get());

        let vars = post_variables_and_rows(&problem, base_doms, &mut self.lp);
        post_bindings(&vars, encoder, &self.ctx, base_doms, &mut self.lp);

        self.lp_posted = true;

        lprelax_log!(
            "Posted LpRelaxProblem ({} variables, {} rows) at decision level {:?} (num events: {}) from the model at level {:?} in {}",
            problem.cols().unwrap().len(),
            problem.rows().len(),
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
        // Quiescence: the reasoners preceding this one (in propagation order) have inferred all they could.
        let quiescent = {
            let num_events = model.trail().num_events();
            let quiescent = num_events == self.num_events;
            self.num_events = num_events;
            quiescent
        };

        if quiescent {
            if self.encoder.is_none() && model.current_decision_level() == DecLvl::ROOT {
                self.build_encoder(model);
            }
            if self.encoder.is_some() && !self.lp_posted && self.search_started {
                self.post_relaxation(model);
            }
        }

        if !self.lp_posted {
            return Ok(());
        }

        self.lp.propagate(model)
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

    fn on_search_start(&mut self) {
        self.search_started = true;
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

/// Adds one column per column of the problem, then one row per row, and returns the mapping from column tags to LP columns.
///
/// With column merging enabled, several tags share a column, and all of them are in the mapping
/// (see [`LpRelaxProblem::col_index`]), so that each of their bindings constrains that one column.
fn post_variables_and_rows(problem: &LpRelaxProblem, doms: &Domains, lp: &mut Lp) -> HashMap<ColTag, LpVar> {
    let problem_cols = problem
        .cols()
        .expect("the problem must be simplified or sealed before being posted");

    let lp_vars = problem_cols
        .iter()
        .map(|tag| lp.create_auxiliary_variable(0, tag.upper_bound().unwrap_or(INT_CST_MAX)))
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
            row.lhs
                .iter()
                .chain(row.rhs.iter())
                .map(|(tag, _)| vars[&tag])
                .all_unique()
        }),
        "a row uses the same LP variable twice: its coefficients should have been summed",
    );

    for row in problem.rows() {
        // `lhs cmp rhs` reads as `expr cmp 0`, posted as `sign * expr <= 0` for each sign `cmp` calls for.
        let row = row.canonical();
        let signs: &[IntCst] = match row.comp_op() {
            ComparisonOp::Le => &[1],
            ComparisonOp::Ge => &[-1],
            ComparisonOp::Eq => &[1, -1],
        };
        for &sign in signs {
            let sum = LpSum::new(row.expr().iter().map(|(tag, coef)| (vars[&tag], sign * coef)));
            lp.add_linear_leq_constraint(sum, sign * row.expr().constant(), Lit::TRUE, doms);
        }
    }

    vars
}

/// Ties the lifted columns (as well as term grounding columns) of the LP to the literals of the main model that decide them.
///
/// Every binding is a half-binding: it constrains the column when the main model's literal becomes
/// entailed, and never the other way round.
fn post_bindings(
    vars: &HashMap<ColTag, LpVar>,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    lp: &mut Lp,
) {
    // Bind lifted presence variables of the LP with corresponding literals in the main CSP:
    // a column is 1 when its literal is true (and present), and 0 when it is false.
    // (A tautological literal's trigger is entailed at the root, while an absurd literal's never fires.)

    let lifted_presences = encoder.iter_sources().flat_map(|(source, trans_ids)| {
        std::iter::once((
            ColTag::PresenceSource(source, None),
            encoder.get_source_prez(source, ctx),
        ))
        .chain(trans_ids.iter().map(|&trans_id| {
            (
                ColTag::PresenceTransition(trans_id, None),
                encoder.transitions.get_prez(trans_id, ctx),
            )
        }))
    });
    for (tag, lit) in lifted_presences {
        let Some(&var) = vars.get(&tag) else { continue };
        lp.add_bound_update_trigger([doms.presence(lit), lit].into(), BoundRestriction::geq(var, 1), doms);
        lp.add_bound_update_trigger((!lit).into(), BoundRestriction::leq(var, 0), doms);
    }

    // Bind term grounding variables of the LP with corresponding literals in the main CSP:
    // a column is 0 as soon as the term's variable cannot take the value `x` making the term equal to the grounding's value.

    for (term, value) in encoder.iter_distinct_term_assignments() {
        let Some(&col_var) = vars.get(&ColTag::TermGround(term, value)) else {
            continue;
        };
        let var = term.variable();
        debug_assert!(!term.is_cst() && var != Var::ZERO);

        let excluding_lits = match var_value_of(term, value) {
            Some(x) => [
                x.checked_add(1).map(|above| var.geq(above)),
                x.checked_sub(1).map(|below| var.leq(below)),
            ],
            // There exists no `x` value that `var` could ever take such that `term = value`
            None => [Some(Lit::TRUE), None],
        };
        for lit in excluding_lits.into_iter().flatten() {
            lp.add_bound_update_trigger(lit.into(), BoundRestriction::leq(col_var, 0), doms);
        }
    }

    // Bind lifted support variables of the LP with corresponding literals in the main CSP.

    for &((out_trans_id, in_trans_id), active) in encoder.supports.unsorted_out() {
        // (`active`: causal link literal)
        let (Some(lit), Some(&var)) = (active, vars.get(&ColTag::Support(out_trans_id, in_trans_id, None))) else {
            continue;
        };
        lp.add_bound_update_trigger((!lit).into(), BoundRestriction::leq(var, 0), doms);

        // NOTE: the converse half-binding, raising the column's lower bound to 1 on an *active* link, is deliberately absent by default !!
        // Indeed, it is *unsound* if condition transitions are allowed to support other transitions (i.e. act as out-transitions).
        // See [`ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS`].
        // This is because we bound the "out-support" of a support by 1, but the main model lets one effect support several conditions,
        // so forcing two of those columns to 1 would make the LP relaxation report a contradiction that the main model does not have.
        //
        // // lp.add_bound_update_trigger([doms.presence(lit), lit], BoundConstraint::geq(col_var, 1), doms);
    }
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
