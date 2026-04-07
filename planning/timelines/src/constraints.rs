use aries::core::literals::{ConjunctionBuilder, Disjunction};
use aries::model::lang::expr::And;
use aries::model::lang::linear::LinearSum;
use aries::prelude::*;
use aries::{
    core::{literals::DisjunctionBuilder, views::Dom},
    lits,
    model::lang::{
        expr::{and, eq, f_geq, f_leq, f_lt, neq, or},
        hreif::{BoolExpr, Store, exclu_choice},
        max::EqMax,
    },
};

use crate::{boxes::Segment, effects::EffectOp, *};

/// Constraint that enforces the [`Sched::makespan`] variable to be equal to the
/// maximum end time of tasks, or zero in the absence of tasks.
pub(crate) struct MakespanIsMaxTaskEnd;
impl BoolExpr<Sched> for MakespanIsMaxTaskEnd {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        assert_eq!(ctx.makespan.denom, ctx.time_scale);
        let mut ends = ctx
            .tasks
            .iter()
            .map(|t| {
                assert_eq!(t.end.denom, ctx.time_scale);
                t.end.num
            })
            .collect_vec();
        ends.push(IAtom::ZERO); // default value when no task is present
        EqMax::new(ctx.makespan.num, ends).enforce_if(l, ctx, store);
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        // constraints is always valid (scope of makespan variable)
        lits![]
    }
}

pub struct NoOverlap(Vec<TaskId>);
impl NoOverlap {
    pub fn new(tasks: Vec<TaskId>) -> Self {
        Self(tasks)
    }
}

impl BoolExpr<Sched> for NoOverlap {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        for (i, t1) in self.0.iter().copied().enumerate() {
            for &t2 in &self.0[(i + 1)..] {
                Mutex(t1, t2).opt_enforce_if(l, ctx, store);
            }
        }
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        lits![]
    }
}

pub struct Mutex(TaskId, TaskId);

impl BoolExpr<Sched> for Mutex {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        let t1 = &ctx.tasks[self.0];
        let t2 = &ctx.tasks[self.1];
        let exclu = exclu_choice(f_leq(t1.end, t2.start), f_leq(t2.end, t1.start));
        exclu.opt_enforce_if(l, ctx, store);
    }

    fn conj_scope(&self, ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        aries::lits![ctx.tasks[self.0].presence, ctx.tasks[self.1].presence]
    }
}

pub(crate) struct EffectCoherence;

impl BoolExpr<Sched> for EffectCoherence {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        for e in ctx.effects.iter() {
            // WARN: this is not guarded by the effect presence (assumption is that this is always true in an effect)
            f_leq(e.transition_start, e.transition_end).opt_enforce_if(l, ctx, store);
            // WARN: this is not guarded by the effect presence (assumption is that that the mutex end has the same scope as the effect)
            f_leq(e.transition_end, e.mutex_end).opt_enforce_if(l, ctx, store);
        }

        // two phases coherence enforcement (between assignments only):
        //  - broad phase: computing a bounding box of the space potentially affected by the effect and gather all overlapping boxes
        //  - for any pair of effects with overlapping bounding boxes, add coherence constraints
        for (eff_id1, eff_id2) in ctx.effects.potentially_interacting_effects() {
            // ensure that the interval `(transition_start, mutex_end]` do not overlap
            let eff1 = &ctx.effects[eff_id1];
            let eff2 = &ctx.effects[eff_id2];

            // this phase only concerns assignments
            let EffectOp::Assign(_) = eff1.operation else {
                continue;
            };
            let EffectOp::Assign(_) = eff2.operation else {
                continue;
            };
            let itv1 = IntervalOnStateVariable {
                state_var: &eff1.state_var,
                start: eff1.transition_start + FAtom::EPSILON,
                end: eff1.mutex_end,
                presence: eff1.prez,
            };
            let itv2 = IntervalOnStateVariable {
                state_var: &eff2.state_var,
                start: eff2.transition_start + FAtom::EPSILON,
                end: eff2.mutex_end,
                presence: eff2.prez,
            };
            let exclu = Exclusive { a: &itv1, b: &itv2 };
            exclu.opt_enforce_if(l, ctx, store);
        }

        // for any step, ensures that:
        //  1) it appears in an assignments exclusivity interval
        //  2) its mutex_end matches this assignments' mutex end
        // Condition 2) is necessary to make sure that any time at which the step contributes to the state variable value is included in its interval
        for step in ctx.effects.iter() {
            let EffectOp::Step(_) = step.operation else {
                continue;
            };
            let compatible_assignemnts = ctx
                .effects
                .potentially_overlapping_effects(&step.state_var.fluent, step.affected_box(&store).as_ref())
                .filter_map(|eid| {
                    let eff = &ctx.effects[eid];
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
                conjuncts.push(f_leq(ass.transition_end, step.transition_start).implicant(ctx, store));
                // note: this forces the `step` interval to exactly match the end of the assignment
                conjuncts.push(f_leq(ass.mutex_end, step.mutex_end).implicant(ctx, store));
                conjuncts.push(f_geq(ass.mutex_end, step.mutex_end).implicant(ctx, store));
                for (arg1, arg2) in ass.state_var.args.iter().zip_eq(step.state_var.args.iter()) {
                    conjuncts.push(eq(*arg1, *arg2).implicant(ctx, store))
                }
                let supports = and(conjuncts.build().into_lits().into_boxed_slice()).implicant(ctx, store);
                support_options.push(supports);
            }
            // if the step it present, then at least one of the assignment must "support it"
            support_options.push(!step.prez);
            support_options.build().enforce_if(l, ctx, store);
        }
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        lits![]
    }
}

#[derive(Debug)]
pub struct HasValueAt {
    pub state_var: StateVar,
    pub value: IAtom,
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
        buff.push(Segment::from(dom.bounds(self.timepoint.num))); // TODO: careful with denom
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
    contribution: IntCst,
}
#[derive(Debug)]
struct AssignEstablisher {
    establishes: Lit,
    base: IntCst,
}

impl BoolExpr<Sched> for HasValueAt {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        let value_box = self.value_box(&ctx.model);

        // gathers all effect that may contribute to the value
        let relevant_effects = ctx
            .effects
            .potentially_supporting_effects(&self.state_var.fluent, value_box.as_ref())
            .map(|eff_id| &ctx.effects[eff_id])
            .collect_vec();

        let mut step_contributors = Vec::new();
        for &eff in &relevant_effects {
            debug_assert_eq!(self.state_var.fluent, eff.state_var.fluent);
            let EffectOp::Step(step) = eff.operation else {
                continue;
            };
            if step == 0 {
                continue;
            }

            let mut conjuncts = ConjunctionBuilder::new();
            conjuncts.push(eff.prez);
            conjuncts.push(f_geq(self.timepoint, eff.effective_start()).reified(ctx, store));
            conjuncts.push(f_leq(self.timepoint, eff.mutex_end).reified(ctx, store));
            for (arg1, arg2) in self.state_var.args.iter().zip_eq(eff.state_var.args.iter()) {
                conjuncts.push(eq(*arg1, *arg2).reified(ctx, store))
            }
            if !conjuncts.absurd() {
                let conjuncts: And = and(conjuncts.build().into_lits().into_boxed_slice()); // TODO: make And = Conjunction
                let contributes = conjuncts.reified(ctx, store); // presence should be the same as self.presence?
                step_contributors.push(StepContributor {
                    contributes,
                    contribution: step,
                });
            }
        }

        // compute assign establisehrs. Those are exclusive (by effect coherence) so half reification is sufficient
        let mut establishers = Vec::with_capacity(16);
        for &eff in &relevant_effects {
            debug_assert_eq!(self.state_var.fluent, eff.state_var.fluent);
            let EffectOp::Assign(assignment) = eff.operation else {
                continue;
            };
            if self.state_var.fluent != eff.state_var.fluent {
                continue;
            }
            let mut conjuncts = ConjunctionBuilder::new();
            conjuncts.push(eff.prez);
            conjuncts.push(f_geq(self.timepoint, eff.effective_start()).implicant(ctx, store));
            conjuncts.push(f_leq(self.timepoint, eff.mutex_end).implicant(ctx, store));
            for (arg1, arg2) in self.state_var.args.iter().zip_eq(eff.state_var.args.iter()) {
                conjuncts.push(eq(*arg1, *arg2).implicant(ctx, store))
            }
            if !conjuncts.absurd() {
                let conjuncts: And = and(conjuncts.build().into_lits().into_boxed_slice()); // TODO: make And = Conjunction
                let establishes = conjuncts.implicant(ctx, store); // presence should be the same as self.presence?
                establishers.push(AssignEstablisher {
                    establishes,
                    base: assignment,
                });
            }
        }

        if step_contributors.is_empty() {
            bind_alternative(l, self.value, self.prez, &establishers, store);
        } else {
            // note: if there are not steps, we can use self.value as the base_variable (which is equivalent to the previous encoding?)

            // Create a `base_variable` that will take the value of the selected establisher
            // has a base_variable = alternative { e in assign_establishers }
            let base_lb = establishers.iter().map(|e| e.base).min().unwrap_or(0);
            let base_ub = establishers.iter().map(|e| e.base).max().unwrap_or(0);
            let base_var: IAtom = store.new_optional_var(base_lb, base_ub, self.prez).into();
            bind_alternative(l, base_var, self.prez, &establishers, store);

            // and self.value = base_variable + Sum { step contirbutions }
            let lhs = LinearSum::from(self.value);
            let mut rhs = LinearSum::from(base_var);
            for step in step_contributors {
                rhs += bool2int(step.contributes, store) * step.contribution;
            }
            lhs.clone().leq(rhs.clone()).enforce(ctx, store);
            lhs.geq(rhs).enforce(ctx, store);
        }

        // PDDL mutex: a condition of an action cannot rely on a fact that is about to be modified by another action
        // given the interval `[cond.start, cond.end]`, we ensure it does not meet the interval `[eff.transition_start, eff.transition_end)`
        // for any effect `eff` with a different source
        let itv_cond = IntervalOnStateVariable {
            state_var: &self.state_var,
            start: self.timepoint,
            end: self.timepoint,
            presence: self.prez,
        };
        for eff_id in ctx
            .effects
            .potentially_overlapping_transitions(&self.state_var.fluent, value_box.as_ref())
        {
            // TODO: mutex when considering steps?
            let eff = &ctx.effects[eff_id];
            if eff.source != self.source {
                let itv_eff = IntervalOnStateVariable {
                    state_var: &eff.state_var,
                    start: eff.transition_start,
                    end: eff.transition_end - FAtom::EPSILON,
                    presence: eff.prez,
                };
                let exclu = Exclusive {
                    a: &itv_cond,
                    b: &itv_eff,
                };
                exclu.opt_enforce_if(l, ctx, store);
            }
        }
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        lits![self.prez]
    }
}

/// Enforce that, if presence is true, then,
///  - exactly one of the alternatives is holds (call it a)
///  - for this alternative a , `value = a.base`
/// ELEMENT
fn bind_alternative(l: Lit, value: IAtom, presence: Lit, alternatives: &[AssignEstablisher], store: &mut dyn Store) {
    // println!("\n\n ===== bind alts ===== \n\n");
    // dbg!(value, presence, alternatives);
    let ctx = &(); // constraints used here are independent of any context, so we just use the unit type

    // at least one esatablisher must hold
    Disjunction::from_iter(alternatives.iter().map(|a| a.establishes)).enforce_if(l, ctx, store);

    for (ai, a) in alternatives.iter().enumerate() {
        // it is exclusive of all other establishers
        // note that is expected to be a redundant constraint (already indirectly enforced by effect coherence)
        for b in &alternatives[ai + 1..] {
            or([!presence, !a.establishes, !b.establishes]).enforce_if(l, ctx, store);
        }

        // if `a` is the establishers the the variable must have its value
        or([!presence, !a.establishes, eq(a.base, value).implicant(ctx, store)]).enforce_if(l, ctx, store);
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
impl<'a> BoolExpr<Sched> for Exclusive<'a> {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        let a = &self.a;
        let b = &self.b;
        debug_assert_eq!(
            a.state_var.fluent, b.state_var.fluent,
            "To expensive to check here, must be filtered earlier"
        );
        let mut disjuncts = DisjunctionBuilder::new();
        for (x1, x2) in a.state_var.args.iter().zip_eq(b.state_var.args.iter()) {
            for opt in neq(*x1, *x2).as_elementary_disjuncts(store) {
                disjuncts.push(opt.implicant(ctx, store));
                if disjuncts.tautological() {
                    return;
                }
            }
        }
        // put last as we are more likely to be able to short circuit on the parameters
        disjuncts.push(f_lt(a.end, b.start).implicant(ctx, store));
        disjuncts.push(f_lt(b.end, a.start).implicant(ctx, store));
        disjuncts.push(!a.presence);
        disjuncts.push(!b.presence);
        if !disjuncts.tautological() {
            or(disjuncts).opt_enforce_if(l, ctx, store);
        }
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        lits![]
    }
}

/// Transforms a boolean into an integer expression
/// NOte: the implementation is currently incomplete
#[doc(hidden)]
pub fn bool2int(b: Lit, model: &mut dyn Store) -> LinearSum {
    let is_zero_one = model.bounds(b.variable()) == (0, 1);
    if model.entails(b) {
        1.into()
    } else if model.entails(!b) {
        0.into()
    } else if is_zero_one && b == b.variable().geq(1) {
        IVar::new(b.variable()).into()
    } else if is_zero_one && b == b.variable().leq(0) {
        LinearSum::constant_int(1) - IVar::new(b.variable()) // TODO: careful, the constant part is optional as well
    } else {
        let bvar = model.new_optional_var(0, 1, model.presence_literal(b));
        eq(bvar.geq(1), b).enforce(&(), model);
        LinearSum::from(bvar)
    }
}
