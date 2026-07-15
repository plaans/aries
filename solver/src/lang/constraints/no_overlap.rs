use crate::lang::exclusive_choice::exclu_choice;
use crate::prelude::*;

use crate::lang::BoolExpr;
use crate::lang::ModelView;
use crate::reasoners::cp::no_overlap::NoOverlapPropagator;
use crate::reasoners::cp::no_overlap::PropagatorKind;
use crate::reasoners::cp::no_overlap::Task;

/// An interval, typically representing the time during which a task executes in scheduling.
///
/// The presence of the interval is implicitly represented as the presence of the `start` variable of the interval.
#[derive(Debug, Clone)]
pub struct Interval {
    /// Variable denoting the start time of the interval
    pub start: VarCst,
    /// Variable denoting the duration of the interval
    pub duration: VarCst,
    /// Variable denoting the end time of the interval
    pub end: VarCst,
}

impl Interval {
    /// Creates a an interval with a variable duration.
    ///
    /// It is assumed that `start + duration = end` always holds and the presence of the interval is determined by the
    /// presence of the start variable.
    pub fn new(start: impl Into<VarCst>, duration: impl Into<VarCst>, end: impl Into<VarCst>) -> Self {
        Self {
            start: start.into(),
            duration: duration.into(),
            end: end.into(),
        }
    }

    /// Creates a new interval with a fixed duration.
    ///
    /// The presence of the interval is determined by the presence of the `start` variable.
    pub fn new_fixed_duration(start: impl Into<VarCst>, duration: impl Into<IntCst>) -> Self {
        let start = start.into();
        let duration = duration.into();
        Self::new(start, duration, start + duration)
    }
}

/// Requires that any two present intervals do not overlap in time.
///
/// For all distinct intervals `i` and `j`: `end(i) <= start(j) OR end(j) <= start(j)`.
/// Note that, as usual in scheduling, in interval is allowed to start exactly when the other ends.
///
/// ## Optional variables
///
/// Intervals that are absent are ignored (i.e. they may not overlap with any other interval).
/// For all distinct intervals `i` and `j`: `!prez(i) OR !prez(j) OR end(i) <= start(j) OR end(j) <= start(j)`.
pub struct NoOverlap {
    intervals: Vec<Interval>,
    edge_finding_propagation: PropagatorKind,
}

impl NoOverlap {
    pub fn new(intervals: Vec<Interval>) -> Self {
        Self {
            intervals,
            edge_finding_propagation: PropagatorKind::default(),
        }
    }
}

impl<Ctx> BoolExpr<Ctx> for NoOverlap
where
    Ctx: ModelView,
{
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        for (i, ti) in self.intervals.iter().enumerate() {
            // presence of start determines the presence of
            debug_assert_eq!(ctx.presence(ti.start), ctx.presence(ti.end));
            ctx.add_assertion(implies(ctx.presence(ti.start), ctx.presence(ti.duration)));
            ctx.add_assertion(ti.duration.ge_lit(0));
            ctx.add_assertion(eq(LinSum::from(ti.start) + ti.duration, ti.end));

            for tj in &self.intervals[i + 1..] {
                exclu_choice(leq(ti.end, tj.start), leq(tj.end, ti.start)).opt_enforce_if(implicant, ctx);
            }
        }

        let propagator: NoOverlapPropagator<VarCst> = NoOverlapPropagator::new(
            self.intervals
                .iter()
                .map(|itv| Task::new(itv.start, itv.duration, itv.end, ctx.presence(itv.start))),
        );
        ctx.enforce_user_propagator(propagator.with_kind(self.edge_finding_propagation));
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        // constraint is always in scope (absent items are ignored)
        Lit::TRUE.into()
    }
}
