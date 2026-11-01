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
/// The LP relaxation problem is posted to the reasoner
/// once the search has started and the model is fully propagate (quiescence),
/// i.e. before the first decision.
///
/// It is posted in two steps:
/// - 1st, the [`LpRelaxEncoder`] is built against the domains of the root level,
/// - 2nd, it builds the LP relaxation problem, and simplifies it with the same root domains
///   (*NOT* the domains right before the first decision).
///   It then binds the reasoner (the LP's columns) to the main model (events on its literals).
///
/// This avoids building and solving the LP if the main model can be proven unsatisfiable (or solved) without search.
///
/// Once posted, the LP is updated and incrementally checked at every propagation by the reasoner itself,
/// with limited effort (see the pivot budget in [`LpOptions`]).
#[derive(Clone)]
pub(crate) struct LpRelaxIncr {
    lp: Lp,
    ctx: SchedEncoder,

    /// Whether the search has started, i.e. all assumptions are posted. The relaxed problem is only built afterwards.
    search_started: bool,
    /// The number of events of the model at the previous propagation, to detect quiescence before posting (`None` once posted).
    num_events: Option<u32>,
}

impl LpRelaxIncr {
    /// The identity of the wrapper, also given to the LP reasoner so that the solver asks the wrapper for its explanations.
    const IDENTITY: ReasonerId = ReasonerId::Extra(42);

    pub(crate) fn new(ctx: SchedEncoder, lp_options: LpOptions) -> Self {
        let mut lp = Lp::new(Self::IDENTITY, lp_options);
        lp.enable();

        Self {
            lp,
            ctx,
            search_started: false,
            num_events: Some(0),
        }
    }

    /// Stage 1: collect the transitions and their supports (pruned with the domains of the root level).
    fn build_encoder(&self, root_doms: &Domains) -> LpRelaxEncoder {
        debug_assert_eq!(root_doms.current_decision_level(), DecLvl::ROOT);

        let time = std::time::Instant::now();

        let encoder = LpRelaxEncoder::with_transitions_from(&self.ctx, root_doms);

        lprelax_log!(
            "Built LpRelaxEncoder at decision level {:?} (num events: {}) in {}",
            root_doms.current_decision_level(),
            root_doms.num_events(),
            time.elapsed().as_secs_f64(),
        );

        encoder
    }

    /// Stage 2: ground and encode the problem, simplify it (with the domains of the root level),
    /// and hand both it and its bindings to the reasoner.
    fn post_relaxation(&mut self, encoder: &mut LpRelaxEncoder, root_doms: &Domains, doms: &Domains) {
        let time = std::time::Instant::now();

        let mut problem = encoder.encode(&self.ctx);
        problem.seal_simplified(encoder, &self.ctx, root_doms, ARIES_LPRELAX_MERGE_EQUAL_COLUMNS.get());

        let vars = post_variables_and_rows(&problem, root_doms, &mut self.lp);
        post_bindings(&vars, encoder, &self.ctx, root_doms, &mut self.lp);

        let elapsed = time.elapsed().as_secs_f64();

        lprelax_log!(
            "Posted LpRelaxProblem ({} variables, {} rows, {} nonzeros) at decision level {:?} (num events: {}) from the root model in {}",
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
            elapsed,
        );
    }
}

impl Theory for LpRelaxIncr {
    fn identity(&self) -> ReasonerId {
        Self::IDENTITY
    }

    fn propagate_at_quiescence_only(&self) -> bool {
        self.lp.propagate_at_quiescence_only()
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        let lp_posted = self.num_events.is_none();
        if !lp_posted {
            // quiescence: no event since the previous propagation, so the reasoners preceding this one have inferred all they could
            let num_events = model.trail().num_events();
            let quiescent = self.num_events == Some(num_events);
            self.num_events = Some(num_events);
            if !(self.search_started && quiescent) {
                return Ok(());
            }

            // posted once, at the first quiescence on the search's start
            self.num_events = None;
            let mut root_doms = model.clone();
            root_doms.restore(DecLvl::ROOT);

            let mut encoder = self.build_encoder(&root_doms);
            self.post_relaxation(&mut encoder, &root_doms, model);
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
