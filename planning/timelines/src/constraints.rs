pub mod symmetry;

use aries_solver::core::literals::ConjunctionBuilder;
use aries_solver::lang::element::Element;
use aries_solver::lang::exclusive_choice::exclu_choice;
use aries_solver::lang::expr::{And, geq, implies, leq, lin_eq, lin_geq, lin_gt, lin_leq, lin_lt, lt};
use aries_solver::prelude::*;
use aries_solver::{
    core::{literals::DisjunctionBuilder, views::Dom},
    lang::{expr::or, max::EqMax},
};

use crate::{boxes::Segment, effects::EffectOp, *};

/// Constraint that enforces the [`Sched::makespan`] variable to be equal to the
/// maximum end time of tasks, or zero in the absence of tasks.
///
/// It is enforced by default in [`Sched`].
#[derive(Debug)]
pub(crate) struct MakespanIsMaxTaskEnd;

impl BoolExpr<SchedEncoder> for MakespanIsMaxTaskEnd {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        let _span = tracing::debug_span!("MakespanIsMaxTaskEnd");
        let _span = _span.enter();
        let mut ends = ctx.sched.tasks.iter().map(|t| t.end).collect_vec();
        ends.push(IAtom::ZERO); // default value when no task is present
        EqMax::new(ctx.sched.makespan, ends).enforce_if(l, ctx);

        // enforce the horizon to be after the end of all actions
        leq(ctx.sched.makespan, ctx.sched.horizon).enforce_if(l, ctx);
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> Conjunction {
        // constraints is always valid (scope of makespan variable)
        Conjunction::tautology()
    }
}

pub struct NoOverlap(Vec<TaskId>);
impl NoOverlap {
    pub fn new(tasks: Vec<TaskId>) -> Self {
        Self(tasks)
    }
}

impl BoolExpr<SchedEncoder> for NoOverlap {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        for (i, t1) in self.0.iter().copied().enumerate() {
            for &t2 in &self.0[(i + 1)..] {
                Mutex(t1, t2).opt_enforce_if(l, ctx);
            }
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> Conjunction {
        Conjunction::tautology()
    }
}

pub struct Mutex(TaskId, TaskId);

impl BoolExpr<SchedEncoder> for Mutex {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        let t1 = &ctx.sched.tasks[self.0];
        let t2 = &ctx.sched.tasks[self.1];
        let exclu = exclu_choice(leq(t1.end, t2.start), leq(t2.end, t1.start));
        exclu.opt_enforce_if(l, ctx);
    }

    fn conj_scope(&self, ctx: &SchedEncoder) -> Conjunction {
        [ctx.sched.tasks[self.0].presence, ctx.sched.tasks[self.1].presence].into()
    }
}

/// Ensures all effects are coherent (enforced by default in [`Sched`]).
///
/// This requires to conditions
///  - that no two assignments have overlapping exclusivity periods
///  - that every step is within an assignment validity period
#[derive(Debug)]
pub(crate) struct EffectCoherence;

impl BoolExpr<SchedEncoder> for EffectCoherence {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        let _span = tracing::debug_span!("EffectCoherence");
        let _span = _span.enter();
        let sched = ctx.sched.clone();
        for e in sched.effects.iter() {
            // WARN: this is not guarded by the effect presence (assumption is that this is always true in an effect)
            leq(e.transition_start, e.transition_end).opt_enforce_if(l, ctx);
            // WARN: this is not guarded by the effect presence (assumption is that that the mutex end has the same scope as the effect)
            leq(e.transition_end, e.mutex_end).opt_enforce_if(l, ctx);

            // enforce that the horizon is after all effects
            leq(e.mutex_end, ctx.sched.horizon).opt_enforce_if(l, ctx);
        }

        // two phases coherence enforcement (between assignments only):
        //  - broad phase: computing a bounding box of the space potentially affected by the effect and gather all overlapping boxes
        //  - for any pair of effects with overlapping bounding boxes, add coherence constraints
        for (eff_id1, eff_id2) in sched.effects.potentially_interacting_effects() {
            // ensure that the interval `(transition_start, mutex_end]` do not overlap
            let eff1 = &sched.effects[eff_id1];
            let eff2 = &sched.effects[eff_id2];

            // this phase only concerns assignments
            let EffectOp::Assign(_) = eff1.operation else {
                continue;
            };
            let EffectOp::Assign(_) = eff2.operation else {
                continue;
            };
            let itv1 = IntervalOnStateVariable {
                state_var: &eff1.state_var,
                start: eff1.transition_start + ctx.sched.epsilon,
                end: eff1.mutex_end,
                presence: eff1.prez,
            };
            let itv2 = IntervalOnStateVariable {
                state_var: &eff2.state_var,
                start: eff2.transition_start + ctx.sched.epsilon,
                end: eff2.mutex_end,
                presence: eff2.prez,
            };
            let exclu = Exclusive { a: &itv1, b: &itv2 };
            exclu.opt_enforce_if(l, ctx);
        }

        // for any 'step', ensures that:
        //  1) it appears in an assignments exclusivity interval
        //  2) its mutex_end matches this assignments' mutex end
        // Condition 2) is necessary to make sure that any time at which the step contributes to the state variable value is included in its interval
        for step in sched.effects.iter() {
            let EffectOp::Step(_) = step.operation else {
                continue;
            };
            let compatible_assignemnts = ctx
                .sched
                .effects
                .potentially_overlapping_effects(&step.state_var.fluent, step.affected_box(&ctx).as_ref())
                .filter_map(|eid| {
                    let eff = &sched.effects[eid];
                    match eff.operation {
                        EffectOp::Assign(_) => Some(eff),
                        EffectOp::Step(_) => None,
                    }
                })
                .collect_vec();

            let mut support_options = DisjunctionBuilder::new();

            for ass in compatible_assignemnts {
                let mut conjuncts = ConjunctionBuilder::new();
                conjuncts.push(ass.prez);
                conjuncts.push(leq(ass.transition_end, step.transition_start).implicant(ctx));
                // note: this forces the `step` interval to exactly match the end of the assignment
                conjuncts.push(leq(ass.mutex_end, step.mutex_end).implicant(ctx));
                conjuncts.push(geq(ass.mutex_end, step.mutex_end).implicant(ctx));
                for (arg1, arg2) in ass.state_var.args.iter().zip_eq(step.state_var.args.iter()) {
                    conjuncts.push(lin_eq(*arg1, *arg2).implicant(ctx))
                }
                let supports = conjuncts.build().implicant(ctx);
                support_options.push(supports);
            }
            // if the step it present, then at least one of the assignment must "support it"
            support_options.push(!step.prez);
            support_options.build().enforce_if(l, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> Conjunction {
        Conjunction::tautology()
    }
}

#[derive(Clone, Debug)]
pub struct HasValueAt {
    pub state_var: StateVar,
    pub value: IntTerm,
    pub timepoint: Time,
    /// Presence of the condition. Must imply the presence of all variables appearing in it.
    pub prez: Lit,
    /// Specifies if this condition originates from a particular task.
    /// This is used to enforce the PDDL-mutex constraint that specifies
    /// that an aciton must not rely on a value that is immediately delete by *another* action.
    /// (mutex conditions).
    pub source: Option<TaskId>,
}

impl HasValueAt {
    /// Returns a box capturing when and what may be the value required by this condition.
    pub fn value_box(&self, dom: impl Dom) -> crate::boxes::BBox {
        let mut buff = Vec::with_capacity(self.state_var.args.len() + 2);
        buff.push(Segment::from(dom.bounds(self.timepoint)));
        for arg in &self.state_var.args {
            buff.push(Segment::from(dom.bounds(*arg)));
        }
        buff.push(Segment::from(dom.bounds(self.value)));
        crate::boxes::BBox::new(buff)
    }
}

#[derive(Debug)]
struct StepContributor {
    contributes: Lit,
    contribution: IntTerm,
}

impl BoolExpr<SchedEncoder> for HasValueAt {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        ctx.add_assertion(implies(ctx.presence_literal(l), self.prez));
        let _span = tracing::debug_span!("HasValueAt");
        let _span = _span.enter();
        tracing::debug!("{l:?} => {self:?}");
        // cheap clone to please the borrow checker
        let sched: std::sync::Arc<Sched> = ctx.sched.clone();

        let value_box = self.value_box(&*ctx);

        // all effects (assign or steps) that may contribute to the value
        let relevant_effects = sched
            .effects
            .potentially_supporting_effects(&self.state_var.fluent, value_box.as_ref())
            .map(|eff_id| (eff_id, &sched.effects[eff_id]))
            .collect_vec();

        let mut supports = Vec::new();

        // gather all step effects that may contribute and
        // create a literal that it is true iff it does contribute
        //   - effect is present, and
        //   - condition within activity period, and
        //   - same state variable
        let mut step_contributors = Vec::new();
        for &(eff_id, eff) in &relevant_effects {
            let _span = tracing::debug_span!("Step");
            let _span = _span.enter();
            debug_assert_eq!(self.state_var.fluent, eff.state_var.fluent);
            let EffectOp::Step(step) = eff.operation else {
                continue;
            };
            if step == IntTerm::ZERO {
                continue;
            }

            let mut conjuncts = ConjunctionBuilder::new();
            conjuncts.push(eff.prez);
            conjuncts.push(geq(self.timepoint, eff.effective_start()).reified(ctx));
            conjuncts.push(leq(self.timepoint, eff.mutex_end).reified(ctx));
            for (arg1, arg2) in self.state_var.args.iter().zip_eq(eff.state_var.args.iter()) {
                conjuncts.push(lin_eq(*arg1, *arg2).reified(ctx))
            }
            if !conjuncts.absurd() {
                let conjuncts: And = conjuncts.build();
                let contributes = conjuncts.reified(ctx); // presence should be the same as self.presence?
                step_contributors.push(StepContributor {
                    contributes,
                    contribution: step,
                });
                supports.push((eff_id, contributes));
            }
        }

        // compute assign establisehrs. Those are exclusive (by effect coherence) so half reification is sufficient
        let mut establishers = Element::new();
        for &(eff_id, eff) in &relevant_effects {
            let _span = tracing::debug_span!("Establisher");
            let _span = _span.enter();
            debug_assert_eq!(self.state_var.fluent, eff.state_var.fluent);
            let EffectOp::Assign(assignment) = eff.operation else {
                continue;
            };
            if self.state_var.fluent != eff.state_var.fluent {
                continue;
            }
            let mut conjuncts = ConjunctionBuilder::new();
            conjuncts.push(eff.prez);
            conjuncts.push(geq(self.timepoint, eff.effective_start()).implicant(ctx));
            conjuncts.push(leq(self.timepoint, eff.mutex_end).implicant(ctx));
            for (arg1, arg2) in self.state_var.args.iter().zip_eq(eff.state_var.args.iter()) {
                // note we use the conjunctive form with bot leq and geq to avoid reification of the equality
                conjuncts.push(lin_leq(*arg1, *arg2).implicant(ctx));
                conjuncts.push(lin_geq(*arg1, *arg2).implicant(ctx));
            }
            if !conjuncts.absurd() {
                let conjuncts: And = conjuncts.build();
                let establishes = conjuncts.implicant(ctx); // presence should be the same as self.presence?
                ctx.add_assertion(or([!self.prez, ctx.presence_literal(establishes), !establishes]));
                establishers.add_option(establishes, assignment);
                supports.push((eff_id, establishes));
            }
        }

        ctx.causal_links.add_new_condition_participants(self.source, supports);

        {
            let _span = tracing::debug_span!("main");
            let _span = _span.enter();
            if step_contributors.is_empty() {
                // there are no steps, we can use self.value as the base_variable (which is equivalent to the previous encoding?)
                establishers.enforce_eq_if(l, self.value, ctx);
            } else {
                // Create a `base_variable` that will take the value of the selected establisher
                // has a base_variable = alternative { e in assign_establishers }
                let base_var = establishers.reify([self.prez], ctx);

                // and self.value = base_variable + Sum { step contirbutions }
                let lhs = IntExp::from(self.value);
                let mut rhs = IntExp::from(base_var);
                for step in step_contributors {
                    if let Ok(contribution) = IntCst::try_from(step.contribution) {
                        rhs += bool2int(step.contributes, ctx) * contribution;
                    } else {
                        let value =
                            Element::build(&[(step.contributes, step.contribution), (!step.contributes, 0.into())]);
                        rhs += value.reify([self.prez], ctx)
                    }
                }
                lhs.clone().leq(rhs.clone()).enforce(ctx);
                lhs.geq(rhs).enforce(ctx);
            }
        }

        {
            let _span = tracing::debug_span!("PDDL Mutex");
            let _span = _span.enter();
            // PDDL mutex: a condition of an action cannot rely on a fact that is about to be modified by another action
            // given the interval `[cond.start, cond.end]`, we ensure it does not meet the interval `[eff.transition_start, eff.transition_end)`
            // for any effect `eff` with a different source
            let itv_cond = IntervalOnStateVariable {
                state_var: &self.state_var,
                start: self.timepoint,
                end: self.timepoint,
                presence: self.prez,
            };
            for eff_id in sched
                .effects
                .potentially_overlapping_transitions(&self.state_var.fluent, value_box.as_ref())
            {
                // TODO: mutex when considering steps?
                let eff = &sched.effects[eff_id];
                if eff.source != self.source {
                    let itv_eff = IntervalOnStateVariable {
                        state_var: &eff.state_var,
                        start: eff.transition_start,
                        end: eff.transition_end - sched.epsilon,
                        presence: eff.prez,
                    };
                    let exclu = Exclusive {
                        a: &itv_cond,
                        b: &itv_eff,
                    };
                    exclu.opt_enforce_if(l, ctx);
                }
            }
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> Conjunction {
        [self.prez].into()
    }
}

/// A closed interval `[start, end]` associated to a state variable
#[derive(Debug)]
struct IntervalOnStateVariable<'a> {
    state_var: &'a StateVar,
    start: Time,
    end: Time,
    presence: Lit,
}

/// Enforces that if both intervals are present and on the same state variable,
/// then their should be an epsilon separation between them:
///
/// `!prez1 | !prez2 | sv1 != sv2 | end1 < start2 | end2 < start1`
///
/// Note: it assumed that the two state variable share the same fluent (which is only checked in debug mode to avoid costly stirng comparisons)
struct Exclusive<'a> {
    a: &'a IntervalOnStateVariable<'a>,
    b: &'a IntervalOnStateVariable<'a>,
}
impl<'a, Ctx: Store + Dom> BoolExpr<Ctx> for Exclusive<'a> {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        let a = &self.a;
        let b = &self.b;
        debug_assert_eq!(
            a.state_var.fluent, b.state_var.fluent,
            "To expensive to check here, must be filtered earlier"
        );
        let mut disjuncts = DisjunctionBuilder::new();
        for (x1, x2) in a.state_var.args.iter().zip_eq(b.state_var.args.iter()) {
            disjuncts.push(lin_lt(*x1, *x2).implicant(ctx));
            disjuncts.push(lin_gt(*x1, *x2).implicant(ctx));
            if disjuncts.tautological() {
                return;
            }
        }
        // put last as we are more likely to be able to short circuit on the parameters
        disjuncts.push(lt(a.end, b.start).implicant(ctx));
        disjuncts.push(lt(b.end, a.start).implicant(ctx));
        disjuncts.push(!a.presence);
        disjuncts.push(!b.presence);
        if !disjuncts.tautological() {
            or(disjuncts).opt_enforce_if(l, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        Conjunction::tautology()
    }
}

/// Transforms a boolean into an integer expression
pub fn bool2int<Ctx: Store + Dom>(b: Lit, model: &mut Ctx) -> IntExp {
    let is_zero_one = model.bounds(b.variable()) == (0, 1);
    if model.entails(b) {
        1.into()
    } else if model.entails(!b) {
        0.into()
    } else if is_zero_one && b == b.variable().geq(1) {
        b.variable().into()
    } else if is_zero_one && b == b.variable().leq(0) {
        IntExp::cst(1) - b.variable()
    } else {
        let bvar = model.new_optional_var(0, 1, model.presence_literal(b));
        implies(bvar.geq(1), b).enforce(model);
        implies(b, bvar.geq(1)).enforce(model);
        IntExp::from(bvar)
    }
}
