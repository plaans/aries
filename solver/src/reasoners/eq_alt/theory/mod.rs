mod cause;
mod check;
mod explain;
mod propagate;

use std::{
    cell::{RefCell, RefMut},
    collections::VecDeque,
};

use cause::ModelUpdateCause;
use itertools::Itertools;

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor},
    core::{
        state::{Domains, DomainsSnapshot, Explanation, InferenceCause},
        Lit, VarRef,
    },
    reasoners::{
        eq_alt::{
            constraints::{ActivationEvent, Constraint, ConstraintStore},
            graph::DirectedEqualityGraph,
            node::Node,
            relation::EqRelation,
        },
        stn::theory::Identity,
        Contradiction, ReasonerId, Theory,
    },
};

type ModelEvent = crate::core::state::Event;

/// An alternative theory propagator for equality logic.
#[derive(Clone)]
pub struct AltEqTheory {
    constraint_store: ConstraintStore,
    /// Directed graph containing valid and active edges
    enabled_graph: DirectedEqualityGraph,
    /// A cursor that lets us track new events since last propagation
    model_events: ObsTrailCursor<ModelEvent>,
    /// A temporary vec of newly created, unpropagated constraints
    new_constraints: VecDeque<ActivationEvent>,
    identity: Identity<ModelUpdateCause>,
    stats: RefCell<Stats>,
}

impl AltEqTheory {
    pub fn new() -> Self {
        AltEqTheory {
            constraint_store: Default::default(),
            enabled_graph: DirectedEqualityGraph::new(),
            model_events: Default::default(),
            new_constraints: Default::default(),
            identity: Identity::new(ReasonerId::Eq(0)),
            stats: Default::default(),
        }
    }

    /// Add l => (a = b) constraint. a must be a variable, but b can also be a constant
    pub fn add_half_reified_eq_edge(&mut self, l: Lit, a: VarRef, b: impl Into<Node>, model: &Domains) {
        self.add_edge(l, a, b, EqRelation::Eq, model);
    }

    /// Add l => (a != b) constraint, a must be a variable, but b can also be a constant
    pub fn add_half_reified_neq_edge(&mut self, l: Lit, a: VarRef, b: impl Into<Node>, model: &Domains) {
        self.add_edge(l, a, b, EqRelation::Neq, model);
    }

    fn add_edge(&mut self, l: Lit, a: VarRef, b: impl Into<Node>, relation: EqRelation, model: &Domains) {
        let b = b.into();
        let pa = model.presence(a);
        let pb = model.presence(b);

        // When pb => pa, edge a -> b is always valid
        // given that `pa & pb <=> edge_valid`, we can infer that the propagator becomes valid
        // (i.e. `pb => edge_valid` holds) when `pa` becomes true
        let ab_valid = if model.implies(pb, pa) { Lit::TRUE } else { pa };
        let ba_valid = if model.implies(pa, pb) { Lit::TRUE } else { pb };

        // Create and record propagators
        let (ab_prop, ba_prop) = Constraint::new_pair(a.into(), b, relation, l, ab_valid, ba_valid);
        for prop in [ab_prop, ba_prop] {
            self.stats().constraints += 1;

            // Constraints that can never be enabled can be ignored
            if model.entails(!prop.enabler.active) || model.entails(!prop.enabler.valid) {
                continue;
            }
            let id = self.constraint_store.add_constraint(prop.clone());

            if !model.entails(prop.enabler.valid) {
                self.constraint_store.add_watch(id, prop.enabler.valid);
            }

            if !model.entails(prop.enabler.active) {
                self.constraint_store.add_watch(id, prop.enabler.active);
            }

            if model.entails(prop.enabler.valid) && model.entails(prop.enabler.active) {
                // Propagator always active and valid, only need to propagate once
                // So don't add watches
                self.new_constraints.push_back(ActivationEvent::new(id));
            }
        }
    }

    fn stats(&self) -> RefMut<'_, Stats> {
        self.stats.borrow_mut()
    }
}

impl Default for AltEqTheory {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for AltEqTheory {
    fn save_state(&mut self) -> DecLvl {
        assert!(self.new_constraints.is_empty());
        self.enabled_graph.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.enabled_graph.num_saved()
    }

    fn restore_last(&mut self) {
        self.enabled_graph.restore_last();
    }
}

impl Theory for AltEqTheory {
    fn identity(&self) -> ReasonerId {
        self.identity.writer_id
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        // Propagate newly created constraints
        while let Some(event) = self.new_constraints.pop_front() {
            self.propagate_edge(model, event.prop_id)?;
        }

        while let Some(&event) = self.model_events.pop(model.trail()) {
            // Optimisation: If we deactivated an edge with literal l due to a neq cycle, the propagator with literal !l (from reification) is redundant
            if let Some(cause) = event.cause.as_external_inference() {
                if cause.writer == self.identity() && matches!(cause.payload.into(), ModelUpdateCause::NeqCycle(_)) {
                    self.stats().skipped_events += 1;
                    continue;
                }
            }

            // For each constraint which might be enabled by this event
            for (enabler, prop_id) in self.constraint_store.enabled_by(event.new_literal()).collect_vec() {
                // Skip if not enabled
                if !model.entails(enabler.active) || !model.entails(enabler.valid) {
                    continue;
                }
                self.stats().propagations += 1;
                self.propagate_edge(model, prop_id)?;
            }
        }
        Ok(())
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        use ModelUpdateCause::*;

        let cause = ModelUpdateCause::from(context.payload);

        // All explanations require some kind of path
        let path = match cause {
            NeqCycle(constraint_id) => self.neq_cycle_explanation_path(constraint_id, model),
            DomNeq => self.neq_explanation_path(literal, model),
            DomEq => self.eq_explanation_path(literal, model),
            EdgeDeactivation(constraint_id, fwd) => self.deactivation_explanation_path(constraint_id, fwd, model),
        };

        debug_assert!(path.iter().all(|e| model.entails(e.active)));
        self.explain_from_path(model, literal, cause, path, out_explanation);
    }

    fn print_stats(&self) {
        println!("{:#?}", self.stats());
        self.enabled_graph.print_merge_statistics();
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone, Default)]
struct Stats {
    constraints: u32,
    propagations: u32,
    skipped_events: u32,
    // neq_cycle_props: u32,
    eq_props: u32,
    neq_props: u32,
    merges: u32,
    total_paths: u32,
    edges_propagated: u32,
}

#[cfg(test)]
mod tests {
    use crate::{
        collections::seq::Seq,
        core::{
            state::{Cause, InvalidUpdate},
            IntCst,
        },
    };
    use std::fmt::Debug;

    use super::*;

    fn test_with_backtrack<T, F>(mut f: F, eq: &mut AltEqTheory, model: &mut Domains) -> T
    where
        T: Eq + Debug,
        F: FnMut(&mut AltEqTheory, &mut Domains) -> T,
    {
        assert!(
            eq.new_constraints.is_empty(),
            "Cannot test backtrack when activations pending"
        );
        eq.save_state();
        model.save_state();
        let res1 = f(eq, model);
        eq.restore_last();
        model.restore_last();
        let res2 = f(eq, model);
        assert_eq!(res1, res2);
        res1
    }

    impl Domains {
        fn new_bool(&mut self) -> Lit {
            self.new_var(0, 1).geq(1)
        }

        fn cursor_at_end(&self) -> ObsTrailCursor<crate::core::state::Event> {
            let mut cursor = ObsTrailCursor::new();
            cursor.move_to_end(self.trail());
            cursor
        }
    }

    fn expect_explanation(
        mut cursor: ObsTrailCursor<crate::core::state::Event>,
        eq: &mut AltEqTheory,
        model: &Domains,
        lit: Lit,
        expl: impl Into<Explanation>,
    ) {
        let expl: Explanation = expl.into();
        while let Some(e) = cursor.pop(model.trail()) {
            if e.new_literal().entails(lit) {
                let mut out_expl = vec![].into();
                eq.explain(
                    lit,
                    e.cause.as_external_inference().unwrap(),
                    &DomainsSnapshot::preceding(model, lit),
                    &mut out_expl,
                );
                assert_eq!(expl.lits.clone().to_set(), out_expl.lits.to_set())
            }
        }
    }

    #[test]
    fn test_eq_domain_prop() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let a_prez = model.new_bool();
        let b_prez = model.new_bool();
        let a = model.new_optional_var(0, 10, a_prez);
        let b = model.new_optional_var(1, 9, b_prez);
        let c = model.new_var(2, 8);
        let lab = model.new_bool();
        let lbc = model.new_bool();
        let la5 = model.new_bool();

        eq.add_half_reified_eq_edge(lab, a, b, &model);
        eq.add_half_reified_eq_edge(lbc, b, c, &model);
        eq.add_half_reified_eq_edge(la5, a, 5, &model);
        eq.propagate(&mut model).unwrap();

        model.set(b_prez, Cause::Decision).unwrap();
        eq.propagate(&mut model).unwrap();
        assert_eq!(model.bounds(a), (0, 10));
        assert_eq!(model.bounds(b), (1, 9));

        test_with_backtrack(
            |eq, model| {
                model.set(lab, Cause::Decision).unwrap();
                eq.propagate(model).unwrap();
                assert_eq!(model.bounds(a), (1, 9));
                assert_eq!(model.bounds(b), (1, 9));
            },
            &mut eq,
            &mut model,
        );

        test_with_backtrack(
            |eq, model| {
                model.set(lbc, Cause::Decision).unwrap();
                eq.propagate(model).unwrap();
                let cursor = model.cursor_at_end();
                assert_eq!(model.bounds(a), (2, 8));
                assert_eq!(model.bounds(b), (2, 8));
                assert_eq!(model.bounds(c), (2, 8));
                expect_explanation(cursor, eq, model, a.leq(8), vec![lab, lbc, c.leq(8)]);
            },
            &mut eq,
            &mut model,
        );

        test_with_backtrack(
            |eq, model| {
                model.set(la5, Cause::Decision).unwrap();
                let cursor = model.cursor_at_end();
                eq.propagate(model).unwrap();
                assert_eq!(model.bounds(a), (5, 5));
                assert_eq!(model.bounds(b), (2, 8));
                assert_eq!(model.bounds(c), (2, 8));
                expect_explanation(cursor, eq, model, a.leq(5), vec![la5]);
            },
            &mut eq,
            &mut model,
        );
    }

    #[test]
    fn test_neq_domain_prop() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let a_prez = model.new_bool();
        let a = model.new_optional_var(0, 10, a_prez);
        let l1 = model.new_bool();
        let l2 = model.new_bool();
        let l3 = model.new_bool();
        let l4 = model.new_bool();

        eq.add_half_reified_neq_edge(l1, a, 10, &model);
        eq.add_half_reified_neq_edge(l2, a, 0, &model);
        eq.add_half_reified_neq_edge(l3, a, 5, &model);
        eq.add_half_reified_neq_edge(l4, a, 9, &model);

        eq.propagate(&mut model).unwrap();

        test_with_backtrack(
            |eq, model| {
                model.set(l3, Cause::Decision).unwrap();
                eq.propagate(model).unwrap();
                assert_eq!(model.bounds(a), (0, 10));
            },
            &mut eq,
            &mut model,
        );

        test_with_backtrack(
            |eq, model| {
                // FIXME: Swapping these two lines causes test to fail.
                // Need to figure out some solution
                model.set(l1, Cause::Decision).unwrap();
                model.set(l4, Cause::Decision).unwrap();
                model.set(l2, Cause::Decision).unwrap();
                eq.propagate(model).unwrap();
                assert_eq!(model.bounds(a), (1, 8));
            },
            &mut eq,
            &mut model,
        );
    }

    #[test]
    fn test_neq_cycle_prop() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let a = model.new_var(0, 1);
        let b = model.new_var(0, 1);
        let c = model.new_var(0, 1);
        let lab = model.new_bool();
        let lbc = model.new_bool();
        let lca = model.new_bool();
        eq.add_half_reified_eq_edge(lab, a, b, &model);
        eq.add_half_reified_eq_edge(lbc, b, c, &model);
        eq.add_half_reified_neq_edge(lca, c, a, &model);
        eq.propagate(&mut model).unwrap();

        test_with_backtrack(
            |eq, model| {
                let cursor = model.cursor_at_end();
                model.set(lab, Cause::Decision).unwrap();
                model.set(lbc, Cause::Decision).unwrap();
                eq.propagate(model).unwrap();
                assert!(model.entails(!lca));
                expect_explanation(cursor, eq, model, !lca, vec![lab, lbc]);
            },
            &mut eq,
            &mut model,
        );
    }

    #[test]
    fn test_edge_deactivation() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let pres_active_2 = model.new_bool();
        let active1 = model.new_var(1, 1);
        let active2 = model.new_optional_var(0, 1, pres_active_2);
        let pot_var1 = model.new_var(0, 0);
        let pot_l1 = model.new_bool();
        let pres_pot_var2 = model.new_bool();
        model.add_implication(pres_pot_var2, pres_active_2);
        let pot_var2 = model.new_optional_var(1, 1, pres_pot_var2);
        let pot_l2 = model.new_bool();

        eq.add_half_reified_eq_edge(Lit::TRUE, active1, active2, &model);
        eq.add_half_reified_eq_edge(pot_l1, pot_var1, active1, &model);
        eq.add_half_reified_neq_edge(pot_l2, active2, pot_var2, &model);

        eq.propagate(&mut model).unwrap();

        println!("{}", eq.enabled_graph.to_graphviz_grouped());

        // TODO: Need bound propagation to do this
        // assert!(model.entails(!pot_l1));
        assert!(model.entails(!pot_l2));
    }

    #[ignore]
    #[test]
    fn test_grouping() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        // a -==-> b
        let a_pres = model.new_bool();
        let b_pres = model.new_bool();
        model.add_implication(b_pres, a_pres);
        let a = model.new_optional_var(0, 1, a_pres);
        let b = model.new_optional_var(0, 1, b_pres);
        eq.add_half_reified_eq_edge(Lit::TRUE, a, b, &model);

        // b <-==-> c
        let c = model.new_optional_var(0, 1, b_pres);
        eq.add_half_reified_eq_edge(Lit::TRUE, b, c, &model);

        eq.propagate(&mut model).unwrap();

        {
            let g = &eq.enabled_graph;
            let a_id = g.get_id(&a.into()).unwrap();
            let b_id = g.get_id(&b.into()).unwrap();
            let c_id = g.get_id(&c.into()).unwrap();
            assert_eq!(g.get_group_id(b_id), g.get_group_id(c_id));
            assert_ne!(g.get_group_id(a_id), g.get_group_id(b_id));
        }
        // c -==-> d -==-> a
        let d_pres = model.new_bool();
        model.add_implication(d_pres, b_pres);
        model.add_implication(a_pres, d_pres);
        let d = model.new_optional_var(0, 1, d_pres);
        eq.add_half_reified_eq_edge(Lit::TRUE, c, d, &model);
        eq.add_half_reified_eq_edge(Lit::TRUE, d, a, &model);
        eq.propagate(&mut model).unwrap();

        {
            let g = &eq.enabled_graph;
            let a_id = g.get_id(&a.into()).unwrap();
            let b_id = g.get_id(&b.into()).unwrap();
            let c_id = g.get_id(&c.into()).unwrap();
            let d_id = g.get_id(&d.into()).unwrap();
            assert_eq!(g.get_group_id(a_id), g.get_group_id(b_id));
            assert_eq!(g.get_group_id(a_id), g.get_group_id(c_id));
            assert_eq!(g.get_group_id(a_id), g.get_group_id(d_id));
        }

        eq.add_half_reified_eq_edge(Lit::TRUE, a, 1, &model);
        eq.propagate(&mut model).unwrap();
        assert!(model.entails(a.geq(1)));
        assert!(model.entails(b.geq(1)));
        assert!(model.entails(c.geq(1)));
        assert!(model.entails(d.geq(1)));

        let l = model.new_bool();
        eq.add_half_reified_neq_edge(l, a, c, &model);
        eq.propagate(&mut model).unwrap();

        assert!(model.entails(!l));
    }

    #[test]
    fn test_var_neq_const() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let l = model.new_bool();
        let a = model.new_var(9, 10);

        eq.add_half_reified_neq_edge(l, a, 10, &model);

        assert!(eq.propagate(&mut model).is_ok());
        assert_eq!(model.ub(a), 10);

        assert!(model.set(l, Cause::Decision).unwrap_or(false));
        assert!(eq.propagate(&mut model).is_ok());
        assert_eq!(model.ub(a), 9);

        eq.add_half_reified_neq_edge(l, a, 9, &model);
        assert!(eq.propagate(&mut model).is_err_and(
            |e| matches!(e, Contradiction::InvalidUpdate(InvalidUpdate(l,_ )) if l == a.leq(8) || l == a.geq(10))
        ));
    }

    /// l => a != a, infer !l
    #[ignore]
    #[test]
    fn test_neq_self() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();
        let l = model.new_bool();
        let a = model.new_var(0, 1);
        eq.add_half_reified_neq_edge(l, a, a, &model);
        assert!(eq.propagate(&mut model).is_ok());
        assert!(model.entails(!l));
    }

    #[test]
    fn test_propagate() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        // l2 <=> var3 == var4
        // l2 <=> var4 == var5
        // l2 <=> var3 == 1
        // All present
        // Should propagate var5 = 1 when l2 active
        let l2 = model.new_var(0, 1).geq(1);
        let var3 = model.new_var(0, 1);
        let var4 = model.new_var(0, 1);
        let var5 = model.new_var(0, 1);

        test_with_backtrack(
            |eq, model| {
                eq.add_half_reified_eq_edge(l2, var3, var4, model);
                eq.add_half_reified_eq_edge(l2, var4, var5, model);
                eq.add_half_reified_eq_edge(l2, var3, 1 as IntCst, model);

                eq.propagate(model).unwrap();
                assert_eq!(model.lb(var4), 0);
            },
            &mut eq,
            &mut model,
        );

        test_with_backtrack(
            |eq, model| {
                model.set_lb(l2.variable(), 1, Cause::Decision).unwrap();

                eq.propagate(model).unwrap();
                assert_eq!(model.lb(var4), 1);
                assert_eq!(model.lb(var5), 1);
            },
            &mut eq,
            &mut model,
        );
    }

    #[test]
    fn test_propagate_error() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        // l2 <=> var3 == var4
        // l2 <=> var4 == var5
        // l2 <=> var3 == 1
        // All present
        // Should propagate var5 = 1 when l2 active
        let l2 = model.new_var(0, 1).geq(1);
        let var3 = model.new_var(0, 1);
        let var4 = model.new_var(0, 1);
        let var5 = model.new_var(0, 1);

        test_with_backtrack(
            |eq, model| {
                eq.add_half_reified_eq_edge(l2, var3, var4, model);
                eq.add_half_reified_neq_edge(l2, var3, var5, model);
                eq.add_half_reified_eq_edge(l2, var4, var5, model);
                model.set_lb(l2.variable(), 1, Cause::Decision).unwrap();
                eq.propagate(model).expect_err("Contradiction.");
            },
            &mut eq,
            &mut model,
        );
    }

    #[test]
    fn test_with_optionals() {
        // a => b => c <= 1 --> no inference
        // 1 => a => b => c --> inference
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        // let l = model.new_var(0, 1).geq(1);
        let l = Lit::TRUE;
        let c_pres = model.new_var(0, 1).geq(1);
        let b_pres = model.new_var(0, 1).geq(1);
        let a_pres = model.new_var(0, 1).geq(1);
        model.add_implication(c_pres, b_pres);
        model.add_implication(b_pres, a_pres);
        let c = model.new_optional_var(0, 1, c_pres);
        let b = model.new_optional_var(0, 1, b_pres);
        let a = model.new_optional_var(0, 1, a_pres);

        test_with_backtrack(
            |eq, model| {
                eq.add_half_reified_eq_edge(l, a, b, model);
                eq.add_half_reified_eq_edge(l, b, c, model);
                eq.add_half_reified_eq_edge(l, c, 1 as IntCst, model);

                eq.propagate(model).unwrap();

                assert_eq!(model.lb(c), 1);
                assert_eq!(model.lb(b), 0);
                assert_eq!(model.lb(a), 0);
            },
            &mut eq,
            &mut model,
        );

        test_with_backtrack(
            |eq, model| {
                eq.add_half_reified_eq_edge(l, a, 1 as IntCst, model);
                eq.propagate(model).unwrap();

                assert_eq!(model.lb(c), 1);
                assert_eq!(model.lb(b), 1);
                assert_eq!(model.lb(a), 1);
            },
            &mut eq,
            &mut model,
        );
    }

    #[allow(unused)]
    fn test_opt_contradiction() {
        // a => b => c && a !=> c
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let l = Lit::TRUE;
        let c_pres = model.new_var(0, 1).geq(1);
        let b_pres = model.new_var(0, 1).geq(1);
        let a_pres = model.new_var(0, 1).geq(1);

        model.add_implication(c_pres, b_pres);
        model.add_implication(b_pres, a_pres);

        let c = model.new_optional_var(0, 1, c_pres);
        let b = model.new_optional_var(0, 1, b_pres);
        let a = model.new_optional_var(0, 1, a_pres);

        test_with_backtrack(
            |eq, model| {
                eq.add_half_reified_eq_edge(l, a, b, model);
                eq.add_half_reified_eq_edge(l, b, c, model);
                eq.add_half_reified_neq_edge(l, a, c, model);
                eq.propagate(model).expect_err("Contradiction.");
            },
            &mut eq,
            &mut model,
        );
    }

    #[allow(unused)]
    fn test_explanation() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let l1 = model.new_var(9, 10).geq(10);
        let l2 = model.new_var(0, 1).geq(1);
        let c_pres = model.new_var(0, 1).geq(1);
        let b_pres = model.new_var(0, 1).geq(1);
        let a_pres = model.new_var(0, 1).geq(1);

        model.add_implication(c_pres, b_pres);
        model.add_implication(b_pres, a_pres);

        let c = model.new_optional_var(0, 1, c_pres);
        let b = model.new_optional_var(0, 1, b_pres);
        let a = model.new_optional_var(0, 1, a_pres);

        eq.add_half_reified_eq_edge(l1, a, b, &model);
        eq.add_half_reified_eq_edge(l1, b, c, &model);
        eq.save_state();
        model.save_state();
        eq.add_half_reified_neq_edge(l2, a, c, &model);
        model.set_lb(l1.variable(), 10, Cause::Decision);
        let mut cursor = ObsTrailCursor::new();
        while let Some(x) = cursor.pop(model.trail()) {}

        eq.propagate(&mut model)
            .expect("Propagation should work but set l to false");
        assert!(model.entails(!l2));
        assert_eq!(cursor.num_pending(model.trail()), 1);
        let event = cursor.pop(model.trail()).unwrap();
        let expl = &mut vec![].into();
        eq.explain(
            !l2,
            event.cause.as_external_inference().unwrap(),
            &DomainsSnapshot::preceding(&model, !l2),
            expl,
        );
        assert_eq!(expl.lits, vec![l1, l1]);

        // Restore to just a => b => c
        model.restore_last();
        eq.restore_last();

        eq.add_half_reified_eq_edge(Lit::TRUE, a, 1, &model);
        model.set_lb(l1.variable(), 10, Cause::Decision);
        while let Some(x) = cursor.pop(model.trail()) {}
        eq.propagate(&mut model).unwrap();
        assert!(model.entails(c.geq(1)));

        for res in [vec![Lit::TRUE], vec![l1, a.geq(1)], vec![l1, b.geq(1)]] {
            let event = cursor.pop(model.trail()).unwrap();
            let expl = &mut vec![].into();
            eq.explain(
                event.new_literal(),
                event.cause.as_external_inference().unwrap(),
                &DomainsSnapshot::preceding(&model, event.new_literal()),
                expl,
            );
            assert_eq!(expl.lits, res); // 1 => active is enough to explain a >= 1
        }
    }

    // Adding edge propagation breaks this since we infer the same thing in a different way.
    // TODO: Fix
    #[ignore]
    #[test]
    fn test_bug() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let a = model.new_var(10, 11);
        let b = model.new_var(10, 11);
        let l1 = model.new_var(0, 1).geq(1);
        let l2 = model.new_var(0, 1).geq(1);
        let l3 = model.new_var(0, 1).geq(1);
        let l4 = model.new_var(0, 1).geq(1);

        eq.add_half_reified_eq_edge(l1, a, 10, &model);
        eq.add_half_reified_eq_edge(l2, a, 11, &model);
        eq.add_half_reified_eq_edge(l3, b, 10, &model);
        eq.add_half_reified_eq_edge(l4, b, 11, &model);

        model.decide(!l4).unwrap();
        model.decide(l3).unwrap();
        assert!(eq.propagate(&mut model).is_ok());
        model.decide(a.geq(11)).unwrap();
        model.decide(!l2).unwrap();
        model.decide(l1).unwrap();

        let err = eq.propagate(&mut model).unwrap_err();
        assert!(
            matches!(
                err,
                Contradiction::InvalidUpdate(InvalidUpdate(lit, _)) if lit == b.geq(11) || lit == a.leq(10)
            ),
            "Expected InvalidUpdate(b >= 11) or InvalidUpdate(a <= 10), got {:?}",
            err
        );

        let mut expl = vec![].into();
        eq.explain(
            b.geq(11),
            InferenceCause {
                writer: ReasonerId::Eq(0),
                payload: ModelUpdateCause::DomEq.into(),
            },
            &DomainsSnapshot::current(&model),
            &mut expl,
        );

        assert_eq!(expl.lits, vec![l1, l3, a.geq(11)]);
    }

    #[test]
    fn test_bug_2() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();
        let var2 = model.new_var(0, 1);
        let var4 = model.new_var(1, 1);
        eq.add_half_reified_eq_edge(var4.geq(1), var2, 1, &model);
        eq.propagate(&mut model).unwrap();
        assert_eq!(model.lb(var2), 1)
    }

    // var1 != (l) var2
    // var1 == con
    // var2 == con
    // Check !l
    #[test]
    fn test_bug_3() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let var1 = model.new_var(0, 10);
        let var2 = model.new_var(0, 10);
        let con = model.new_var(0, 10);
        let var1_2_l = model.new_bool();
        eq.add_half_reified_eq_edge(Lit::TRUE, var2, con, &model);
        assert!(eq.propagate(&mut model).is_ok());
        eq.add_half_reified_neq_edge(var1_2_l, var1, var2, &model);
        eq.add_half_reified_eq_edge(Lit::TRUE, var1, con, &model);
        assert!(eq.propagate(&mut model).is_ok());
        assert!(model.entails(!var1_2_l));
    }
}
