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

        // two phases coherence enforcement:
        //  - broad phase: computing a bounding box of the space potentially affected by the effect and gather all overlapping boxes
        //  - for any pair of effects with overlapping bounding boxes, add coherence constraints
        for (eff_id1, eff_id2) in ctx.effects.potentially_interacting_effects() {
            // ensure that the interval `(transition_start, mutex_end]` do not overlap
            let eff1 = &ctx.effects[eff_id1];
            let eff2 = &ctx.effects[eff_id2];
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

impl BoolExpr<Sched> for HasValueAt {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        let mut options = Vec::with_capacity(4);

        let value_box = self.value_box(&ctx.model);

        // ensures that at least one effect supports the conditions
        for eff_id in ctx
            .effects
            .potentially_supporting_effects(&self.state_var.fluent, value_box.as_ref())
        {
            let eff = &ctx.effects[eff_id];
            let EffectOp::Assign(value) = eff.operation;
            if self.state_var.fluent != eff.state_var.fluent {
                continue;
            }
            assert_eq!(self.state_var.args.len(), eff.state_var.args.len());
            let mut conjuncts = vec![
                eff.prez,
                f_geq(self.timepoint, eff.effective_start()).implicant(ctx, store),
                f_leq(self.timepoint, eff.mutex_end).implicant(ctx, store),
            ];
            conjuncts.extend(
                self.state_var
                    .args
                    .iter()
                    .zip(eff.state_var.args.iter())
                    .map(|(x, y)| eq(*x, *y).implicant(ctx, store)),
            );
            conjuncts.push(eq(self.value, Atom::from(value)).implicant(ctx, store));

            if conjuncts.iter().all(|c| *c != Lit::FALSE) {
                options.push(and(conjuncts.as_slice()).implicant(ctx, store));
            }
        }
        or(options).opt_enforce_if(l, ctx, store);

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

/// A closed interval `[start, end]` associated to a state variable
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
