use aries::{
    core::literals::DisjunctionBuilder,
    lits,
    model::lang::{
        expr::{and, eq, f_geq, f_leq, neq, or},
        hreif::{BoolExpr, Store, exclu_choice},
    },
};

use crate::{
    effects::{Effect, EffectOp},
    *,
};

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

impl EffectCoherence {
    /// Adds the constraint that if `l` is true, then the two effects should be non-overlapping.
    fn enforce_non_overlapping_if(l: Lit, e: &Effect, e2: &Effect, ctx: &Sched, store: &mut dyn Store) {
        debug_assert_eq!(e.state_var.fluent, e2.state_var.fluent);
        let mut disjuncts = DisjunctionBuilder::new();
        for (x1, x2) in e.state_var.args.iter().zip(e2.state_var.args.iter()) {
            for opt in neq(*x1, *x2).as_elementary_disjuncts(store) {
                disjuncts.push(opt.implicant(ctx, store));
                if disjuncts.tautological() {
                    return;
                }
            }
        }
        // put last as we are more likely to be able to short circuit on the parameters
        disjuncts.push(f_leq(e.mutex_end, e2.transition_start).implicant(ctx, store));
        disjuncts.push(f_leq(e.mutex_end, e2.transition_start).implicant(ctx, store));
        disjuncts.push(!e.prez);
        disjuncts.push(!e2.prez);
        if !disjuncts.tautological() {
            or(disjuncts).opt_enforce_if(l, ctx, store);
        }
    }
}

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
            Self::enforce_non_overlapping_if(l, &ctx.effects[eff_id1], &ctx.effects[eff_id2], ctx, store);
        }
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        lits![]
    }
}

#[derive(Debug)]
pub struct HasValueAt {
    pub state_var: StateVar,
    pub value: Atom,
    pub timepoint: Time,
    /// Presence of the condition. Must imply the presence of all variables appearing in it.
    pub prez: Lit,
}

impl BoolExpr<Sched> for HasValueAt {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        let mut options = Vec::with_capacity(4);

        for eff_id in ctx
            .effects
            .potentially_supporting_effects(self.timepoint, &self.state_var, self.value, |v| ctx.model.int_bounds(v))
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
    }

    fn conj_scope(&self, _ctx: &Sched, _store: &dyn Store) -> hreif::Lits {
        lits![self.prez]
    }
}
