use aries_solver::lang::Store;
use aries_solver::lang::expr::{implies, leq};
use aries_solver::prelude::*;
use aries_solver::{core::literals::DisjunctionBuilder, lang::BoolExpr};
use itertools::Itertools;
use std::collections::{BTreeMap, BTreeSet};

use crate::{TaskId, encoder::SchedEncoder};

#[derive(Debug)]
pub struct SymmetryBreaking {
    kind: SymmetryBreakingKind,
    equivalence_classes: Vec<BTreeSet<TaskId>>,
}

impl SymmetryBreaking {
    pub fn new(kind: SymmetryBreakingKind, equivalence_classes: Vec<BTreeSet<TaskId>>) -> Self {
        Self {
            kind,
            equivalence_classes,
        }
    }
}

/// Kind of symmetry breaking to enforce
#[derive(Debug, Default)]
pub enum SymmetryBreakingKind {
    /// No symmetry breaking
    None,
    /// Place an oredring constraint between all elements of an equivalence class
    ///
    /// This correspond to the symmetry breaking of LCP:
    /// `start_1 <= start_2 <= ... <= start_n`
    ///
    /// Ref (LCP): A constraint-based encoding for domain-independent temporal planning
    Order,
    /// Break symmetries on the causal graph. In essence, this requires that, for an arbitrary order of conditions and effect,
    /// the smallest condition supported by an operation is at least as small that the smallest condition support by later
    #[default]
    CausalGraph,
}

impl BoolExpr<SchedEncoder> for SymmetryBreaking {
    fn enforce_if(&self, implicant: Lit, ctx: &mut SchedEncoder) {
        let sched = ctx.sched.clone();
        match self.kind {
            SymmetryBreakingKind::None => {}
            SymmetryBreakingKind::Order => {
                // operations in the equivalence class should be prioritized (presence and start time) over
                // the ones appearing later in the equivalence class
                for equiv in &self.equivalence_classes {
                    for (&a, &b) in equiv.iter().tuple_windows() {
                        let a = &sched.tasks[a];
                        let b = &sched.tasks[b];
                        implies(b.presence, a.presence).enforce_if(implicant, ctx);
                        leq(a.start, b.start).opt_enforce_if(implicant, ctx);
                    }
                }
            }
            SymmetryBreakingKind::CausalGraph => {
                // This implements the symmetry breaking of (ECAI 25), though the implementation is at this point partial.
                // It works by breaking symmetries on the causal graph, in essence giving higher priority for the the first actions to support conditions
                // (in some arbirary order for action and conditions).
                // The implementation is partial, with 2 TODO items in the implementation that may provide some improvement
                // ECAI 25 ref: Towards Canonical and Minimal Solutions in a Constraint-based Plan-Space Planner

                // gather all all outgoing causal links per action
                let mut links_per_action: BTreeMap<TaskId, Vec<_>> = Default::default();
                let links = ctx.causal_links.get_links().copied().collect_vec();
                for cl in links {
                    let eff = ctx.sched.effects.get(cl.eff);
                    let Some(task_of_effect) = eff.source else {
                        // this is not associated to a task, and thus no in any equivalent class
                        continue;
                    };
                    // remember whether this causal link is exclusive with other of the same king
                    // THis is useful in simplifying a lexical constraint: if literals `a` and `b` are exclusive
                    // the `a < b` is equivalent to `b`. In the general case, you would need to reify `!a & b`
                    let is_exclusive = match &eff.operation {
                        crate::EffectOp::Assign(_) => true,
                        crate::EffectOp::Step(_) => false,
                    };
                    // create a new mandatory literal to capture whether the link is active (requiring also the condition to be present)
                    let supports = Conjunction::from_slice([cl.active, ctx.presence_literal(cl.active)]).reified(ctx);
                    links_per_action
                        .entry(task_of_effect)
                        .or_default()
                        .push((cl.cond, supports, is_exclusive));
                }

                for equiv in &self.equivalence_classes {
                    for (&b, &a) in equiv.iter().tuple_windows() {
                        // creates the signature of the task
                        let supports = |tid: TaskId| {
                            links_per_action[&tid]
                                .iter()
                                // ignore the conditions originating from `a` and `b` that require some special handling otherwise.
                                // This corresponds to (ECAI 25, eq. 3) and slightly weaker than the one propose immediately after.
                                // TODO: implement the complete form (not much more complex but requires some mechanics to handle swapping links between the two signatures)
                                .filter(|cl| cl.0.source != Some(a) && cl.0.source != Some(b))
                                // we sort the elements by their source. This can have a great effect because the earlier
                                // elements are more influential. Here this order only ensures that the condition from the problem (with a `None` source)
                                // are considered first. These are typically the most critical because they are always present and usually ground.
                                //
                                // Note: the sort needs to be stable to ensure the signatures are comparable.
                                //
                                // TODO: The ECAI 25 paper also propose to use abstraction hierarchies to define the order which can bring
                                //       significant performance improvement
                                .sorted_by_key(|cl| cl.0.source) // elements from domain first
                                .collect_vec()
                        };
                        let a_supports = supports(a);
                        let b_supports = supports(b);
                        // ensure that the two signatures are comparable.
                        // The way we get the comparable causal links in the same order is a bit brittle, relying on the fact that the conditions and effects are
                        // posted in the same order for two equivalent actions.
                        // Note that this test does not ensure that the effects are the same
                        // (which is not trivial to do because they are index globally and not relative to the operation's first effect)
                        debug_assert!(
                            a_supports
                                .iter()
                                .zip_eq(b_supports.iter())
                                .all(|(a, b)| a.0 == b.0 && a.2 == b.2),
                        );
                        let lex_items = a_supports
                            .iter()
                            .zip_eq(b_supports.iter())
                            .map(|(a, b)| LexItem {
                                a: a.1,
                                b: b.1,
                                exclusive: a.2,
                            })
                            .collect_vec();
                        let sym_break_constraint = LexLeq { items: lex_items };
                        sym_break_constraint.enforce_if(implicant, ctx);
                    }
                }
            }
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> aries_solver::prelude::Conjunction {
        Lit::TRUE.into()
    }
}

#[derive(Debug)]
struct LexItem {
    a: Lit,
    b: Lit,
    exclusive: bool,
}
impl LexItem {
    pub fn lt<Ctx: Store>(&self, ctx: &mut Ctx) -> Lit {
        if self.exclusive {
            self.b
        } else {
            Conjunction::from([!self.a, self.b]).reified(ctx)
        }
    }
    pub fn le<Ctx: Store>(&self, ctx: &mut Ctx) -> Lit {
        if self.exclusive {
            !self.a
        } else {
            Disjunction::from([!self.a, self.b]).reified(ctx)
        }
    }
}

/// Enforces that the base 2 interpretation of a binary vector is lesser than or equal to another one.
///
/// The constraint supports indicating that two elements are exclusive (can not be true at the time) which enables some
/// more compact encoding in some cases.
///
/// TODO: this could be strengthened and moved to the aries solver as it is a generally useful constraint.
#[derive(Debug)]
struct LexLeq {
    items: Vec<LexItem>,
}

impl<Ctx: Store> BoolExpr<Ctx> for LexLeq {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        for i in 0..self.items.len() {
            let clause = DisjunctionBuilder::from_iter(self.items[..i].iter().map(|item| item.lt(ctx)))
                .with(self.items[i].le(ctx))
                .build();
            clause.enforce_if(implicant, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        // assumes all literals are present.
        // This is ok for the symmetry breaking context but requires deeper thought if made generally available.
        Lit::TRUE.into()
    }
}
