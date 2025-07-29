mod cause;
mod check;
mod explain;
mod propagate;

use std::collections::VecDeque;

use cause::ModelUpdateCause;

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor},
    core::{
        state::{Domains, DomainsSnapshot, Explanation, InferenceCause},
        Lit, VarRef,
    },
    reasoners::{
        eq_alt::{
            graph::DirEqGraph,
            node::Node,
            propagators::{ActivationEvent, Propagator, PropagatorStore},
            relation::EqRelation,
        },
        stn::theory::Identity,
        Contradiction, ReasonerId, Theory,
    },
};

type ModelEvent = crate::core::state::Event;

#[derive(Clone)]
pub struct AltEqTheory {
    constraint_store: PropagatorStore,
    /// Directed graph containt valid and active edges
    active_graph: DirEqGraph,
    model_events: ObsTrailCursor<ModelEvent>,
    pending_activations: VecDeque<ActivationEvent>,
    identity: Identity<ModelUpdateCause>,
}

impl AltEqTheory {
    pub fn new() -> Self {
        AltEqTheory {
            constraint_store: Default::default(),
            active_graph: DirEqGraph::new(),
            model_events: Default::default(),
            pending_activations: Default::default(),
            identity: Identity::new(ReasonerId::Eq(0)),
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
        // Inverse
        let ba_valid = if model.implies(pa, pb) { Lit::TRUE } else { pb };

        // Create and record propagators
        let (ab_prop, ba_prop) = Propagator::new_pair(a.into(), b, relation, l, ab_valid, ba_valid);
        for prop in [ab_prop, ba_prop] {
            if model.entails(!prop.enabler.active) || model.entails(!prop.enabler.valid) {
                continue;
            }
            let id = self.constraint_store.add_propagator(prop.clone());

            if model.entails(prop.enabler.valid) && model.entails(prop.enabler.active) {
                // Propagator always active and valid, only need to propagate once
                // So don't add watches
                self.constraint_store.mark_valid(id);
                self.pending_activations.push_back(ActivationEvent::new(id));
            } else if model.entails(prop.enabler.valid) {
                self.constraint_store.mark_valid(id);
                self.pending_activations.push_back(ActivationEvent::new(id));
                self.constraint_store.watch_propagator(id, prop);
            } else {
                self.constraint_store.watch_propagator(id, prop);
            }
        }
    }
}

impl Default for AltEqTheory {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for AltEqTheory {
    fn save_state(&mut self) -> DecLvl {
        assert!(self.pending_activations.is_empty());
        self.constraint_store.save_state();
        self.active_graph.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.constraint_store.num_saved()
    }

    fn restore_last(&mut self) {
        self.constraint_store.restore_last();
        self.active_graph.restore_last();
    }
}

impl Theory for AltEqTheory {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Eq(0)
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        // println!(
        //     "Before:\n{}\n",
        //     self.active_graph.to_graphviz(),
        //     // self.undecided_graph.to_graphviz()
        // );
        let mut propagated = false;
        while let Some(event) = self.pending_activations.pop_front() {
            propagated = true;
            self.propagate_candidate(model, event.prop_id)?;
        }
        while let Some(event) = self.model_events.pop(model.trail()) {
            for (_, prop_id) in self
                .constraint_store
                .enabled_by(event.new_literal())
                .collect::<Vec<_>>() // To satisfy borrow checker
                .iter()
            {
                propagated = true;
                let prop = self.constraint_store.get_propagator(*prop_id);
                if model.entails(prop.enabler.valid) {
                    self.constraint_store.mark_valid(*prop_id);
                }
                self.propagate_candidate(model, *prop_id)?;
            }
        }
        if propagated {
            // self.check_propagations(model);
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
        // println!("{}", self.active_graph.to_graphviz());
        use ModelUpdateCause::*;

        // Get the path which explains the inference
        let cause = ModelUpdateCause::from(context.payload);
        let path = match cause {
            NeqCycle(prop_id) => self.neq_cycle_explanation_path(prop_id, model),
            DomNeq => self.neq_explanation_path(literal, model),
            DomEq => self.eq_explanation_path(literal, model),
        };

        debug_assert!(path.iter().all(|e| model.entails(e.active)));
        self.explain_from_path(model, literal, cause, path, out_explanation);
    }

    fn print_stats(&self) {
        // self.stats.print_stats();
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
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

    use super::*;

    fn test_with_backtrack<F>(mut f: F, eq: &mut AltEqTheory, model: &mut Domains)
    where
        F: FnMut(&mut AltEqTheory, &mut Domains),
    {
        // TODO: reenable by making sure there are no pending activations when saving state
        // eq.save_state();
        // model.save_state();
        // f(eq, model);
        // eq.restore_last();
        // model.restore_last();
        f(eq, model);
    }

    impl Domains {
        fn new_bool(&mut self) -> Lit {
            self.new_var(0, 1).geq(1)
        }
    }

    fn expect_explanation(
        cursor: &mut ObsTrailCursor<crate::core::state::Event>,
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

    /// 0 <= a <= 10 && l => a == 5
    /// No propagation until l true
    /// l => a == 4 given invalid update
    #[test]
    fn test_var_eq_const() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();
        let mut cursor = ObsTrailCursor::new();
        let l = model.new_bool();
        let a = model.new_var(0, 10);
        eq.add_half_reified_eq_edge(l, a, 5, &model);
        cursor.move_to_end(model.trail());
        assert!(eq.propagate(&mut model).is_ok());
        assert_eq!(model.ub(a), 10);
        assert!(model.set(l, Cause::Decision).unwrap_or(false));
        assert!(eq.propagate(&mut model).is_ok());
        assert_eq!(model.ub(a), 5);
        expect_explanation(&mut cursor, &mut eq, &model, a.leq(5), vec![l]);
        eq.add_half_reified_eq_edge(l, a, 4, &model);
        cursor.move_to_end(model.trail());
        assert!(eq
            .propagate(&mut model)
            .is_err_and(|e| matches!(e, Contradiction::InvalidUpdate(InvalidUpdate(l,_ )) if l == a.leq(4))));
        expect_explanation(&mut cursor, &mut eq, &model, a.leq(4), vec![l]);
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

    /// a -=> b && a -!=> b, infer nothing
    /// when b present, infer !l
    #[test]
    fn test_alt_paths() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();
        let a_pres = model.new_bool();
        let b_pres = model.new_bool();
        model.add_implication(b_pres, a_pres);
        let a = model.new_optional_var(0, 5, a_pres);
        let b = model.new_optional_var(0, 5, b_pres);
        let l = model.new_bool();
        eq.add_half_reified_eq_edge(Lit::TRUE, a, b, &model);
        eq.add_half_reified_neq_edge(l, a, b, &model);
        eq.propagate(&mut model).unwrap();
        assert_eq!(model.bounds(l.variable()), (0, 1));
        model.set(b_pres, Cause::Decision).unwrap();
        dbg!();
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
