use std::collections::HashMap;

use itertools::Itertools;

use aries_linexpr::ComparisonOp;
use aries_solver::backtrack::{Backtrack, DecLvl};
use aries_solver::core::state::{Domains, DomainsSnapshot, Explanation, InferenceCause};
use aries_solver::core::{INT_CST_MAX, IntCst, Lit};
use aries_solver::reasoners::{Contradiction, ReasonerId, Theory};

use aries_solver::reasoners::lp::{BoundRestriction, Lp, LpOptions, LpSum, LpVar};

use crate::SchedEncoder;
use crate::lprelax::ARIES_LPRELAX_MERGE_EQUAL_COLUMNS;
use crate::lprelax::encoder::LpRelaxEncoder;
use crate::lprelax::encoder::problem::{ColTag, LpRelaxProblem};

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

    /// Stage 1: collect the transitions and their supports (pruned with the root domains). Must be done at the root level.
    fn build_encoder(&mut self, doms: &Domains) {
        debug_assert!(self.encoder.is_none() && !self.lp_posted);

        let time = std::time::Instant::now();

        let encoder = LpRelaxEncoder::with_transitions_from(&self.ctx, doms);

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
        let elapsed = time.elapsed().as_secs_f64();

        lprelax_log!(
            "Posted LpRelaxProblem ({} variables, {} rows, {} nonzeros) at decision level {:?} (num events: {}) from the model at level {:?} in {}",
            problem.cols().unwrap().len(),
            problem.rows().len(),
            // (An equality row counts once, though it is posted as two inequalities.)
            problem
                .rows()
                .iter()
                .map(|row| row.canonical().expr().terms().len())
                .sum::<usize>(),
            doms.current_decision_level(),
            doms.num_events(),
            base_decision_level,
            elapsed,
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
        .map(|tag| lp.create_auxiliary_variable(tag.lower_bound(), tag.upper_bound().unwrap_or(INT_CST_MAX)))
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
    let mut fixed: HashMap<IntCst, LpVar> = HashMap::new();
    for (tag, value) in problem.col_known_values() {
        let var = *fixed
            .entry(value)
            .or_insert_with(|| lp.create_auxiliary_variable(value, value));
        vars.insert(tag, var);
    }

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

/// Ties the columns of the LP to the literals of the main model that decide them (see [`LpRelaxEncoder::bindings`]).
fn post_bindings(
    vars: &HashMap<ColTag, LpVar>,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    lp: &mut Lp,
) {
    for binding in encoder.bindings(ctx) {
        let Some(&var) = vars.get(&binding.tag) else {
            continue;
        };
        use crate::lprelax::encoder::bindings::BoundKind;
        let restriction = match binding.bound {
            BoundKind::Lower => BoundRestriction::geq(var, binding.value.into()),
            BoundKind::Upper => BoundRestriction::leq(var, binding.value.into()),
        };
        lp.add_bound_update_trigger(binding.trigger.as_slice().into(), restriction, doms);
    }
}
