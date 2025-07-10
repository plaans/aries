#![allow(unused)]

use std::{collections::VecDeque, fmt::Display};

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail},
    core::{
        state::{Cause, Domains, DomainsSnapshot, Explanation, InferenceCause, InvalidUpdate},
        IntCst, Lit, VarRef,
    },
    reasoners::{
        eq_alt::{
            core::{EqRelation, Node},
            graph::{DirEqGraph, Edge},
            propagators::{Enabler, Propagator, PropagatorId, PropagatorStore},
        },
        stn::theory::Identity,
        Contradiction, ReasonerId, Theory,
    },
};

use super::propagators::ActivationEvent;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
struct EdgeLabel {
    l: Lit,
}

impl Display for EdgeLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.l)
    }
}

impl From<Propagator> for Edge<Node, EdgeLabel> {
    fn from(
        Propagator {
            a,
            b,
            relation,
            enabler: Enabler { active, .. },
        }: Propagator,
    ) -> Self {
        Self::new(a, b, EdgeLabel { l: active }, relation)
    }
}

type ModelEvent = crate::core::state::Event;

#[derive(Clone, Copy)]
enum Event {
    EdgeActivated(PropagatorId),
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum ModelUpdateCause {
    /// Indicates that a propagator was deactivated due to it creating a cycle with relation Neq.
    /// Independant of presence values.
    /// e.g. a -=> b && b -!=> a
    NeqCycle(PropagatorId),
    // DomUpper,
    // DomLower,
    /// Indicates that a bound update was made due to a Neq path being found
    /// e.g. 1 -=> a && a -!=> b && 0 <= b <= 1 implies b < 1
    DomNeq,
    /// Indicates that a bound update was made due to an Eq path being found
    /// e.g. 1 -=> a && a -=> b implies 1 <= b <= 1
    DomEq,
    // Indicates that a
    // DomSingleton,
}

impl From<ModelUpdateCause> for u32 {
    #[allow(clippy::identity_op)]
    fn from(value: ModelUpdateCause) -> Self {
        use ModelUpdateCause::*;
        match value {
            NeqCycle(p) => 0u32 + (u32::from(p) << 1),
            // DomUpper => 1u32 + (0u32 << 1),
            // DomLower => 1u32 + (1u32 << 1),
            DomNeq => 1u32 + (2u32 << 1),
            DomEq => 1u32 + (3u32 << 1),
            // DomSingleton => 1u32 + (4u32 << 1),
        }
    }
}

impl From<u32> for ModelUpdateCause {
    fn from(value: u32) -> Self {
        use ModelUpdateCause::*;
        let kind = value & 0x1;
        let payload = value >> 1;
        match kind {
            0 => NeqCycle(PropagatorId::from(payload)),
            1 => match payload {
                // 0 => DomUpper,
                // 1 => DomLower,
                2 => DomNeq,
                3 => DomEq,
                // 4 => DomSingleton,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Default)]
struct AltEqStats {
    prop_count: u32,
    non_empty_prop_count: u32,
    prop_candidate_count: u32,
    expl_count: u32,
    total_expl_length: u32,
    edge_count: u32,
    any_propped_this_iter: bool,
}

impl AltEqStats {
    fn avg_prop_batch_size(&self) -> f32 {
        self.prop_count as f32 / self.prop_candidate_count as f32
    }

    fn avg_expl_length(&self) -> f32 {
        self.total_expl_length as f32 / self.expl_count as f32
    }

    fn print_stats(&self) {
        println!("Prop count: {}", self.prop_count);
        println!("Average prop batch size: {}", self.avg_prop_batch_size());
        println!("Expl count: {}", self.expl_count);
        println!("Average explanation length: {}", self.avg_expl_length());
    }
}

#[derive(Clone)]
pub struct AltEqTheory {
    constraint_store: PropagatorStore,
    active_graph: DirEqGraph<Node, EdgeLabel>,
    model_events: ObsTrailCursor<ModelEvent>,
    pending_activations: VecDeque<ActivationEvent>,
    trail: Trail<Event>,
    identity: Identity<ModelUpdateCause>,
    stats: AltEqStats,
}

impl AltEqTheory {
    pub fn new() -> Self {
        AltEqTheory {
            constraint_store: Default::default(),
            active_graph: DirEqGraph::new(),
            model_events: Default::default(),
            trail: Default::default(),
            pending_activations: Default::default(),
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
        self.stats.edge_count += 1;
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
        let ab_enabler = ab_prop.enabler;
        let ba_enabler = ba_prop.enabler;
        let ab_id = self.constraint_store.add_propagator(ab_prop);
        let ba_id = self.constraint_store.add_propagator(ba_prop);
        self.active_graph.add_node(a.into());
        self.active_graph.add_node(b);

        // If the propagator is immediately valid, add to queue to be propagated
        // active is not required, since we can set inactive preemptively
        if model.entails(ab_valid) {
            self.pending_activations
                .push_back(ActivationEvent::new(ab_id, ab_enabler));
        }
        if model.entails(ba_valid) {
            self.pending_activations
                .push_back(ActivationEvent::new(ba_id, ba_enabler));
        }

        // If b is a constant, we can add negative edges which all other different constants
        // This avoid 1 -=> 2 being valid
    }

    /// Given an edge that is both active and valid but not added to the graph
    /// check all new paths a -=> b that will be created by this edge, and infer b's bounds from a
    fn propagate_bounds(&mut self, model: &mut Domains, edge: Edge<Node, EdgeLabel>) -> Result<(), InvalidUpdate> {
        // Get all new node pairs we can potentially propagate
        self.active_graph
            .paths_requiring(edge)
            .map(|p| -> Result<(), InvalidUpdate> {
                // Propagate between node pair
                match p.relation {
                    EqRelation::Eq => {
                        self.propagate_eq(model, p.source, p.target)?;
                    }
                    EqRelation::Neq => {
                        self.propagate_neq(model, p.source, p.target)?;
                    }
                };
                Ok(())
            })
            // Stop at first error
            .find(|x| x.is_err())
            .unwrap_or(Ok(()))
    }

    /// Given any propagator, perform propagations if possible and necessary.
    fn propagate_candidate(
        &mut self,
        model: &mut Domains,
        enabler: Enabler,
        prop_id: PropagatorId,
    ) -> Result<(), Contradiction> {
        // If a propagator is definitely inactive, nothing can be done
        if (!model.entails(!enabler.active)
            // If a propagator is not valid, nothing can be done
            && model.entails(enabler.valid)
            // If a propagator is already enabled, all possible propagations are already done
            && !self.constraint_store.is_enabled(prop_id))
        {
            self.stats.prop_candidate_count += 1;
            // Get propagator info
            let prop = self.constraint_store.get_propagator(prop_id);
            let edge: Edge<_, _> = prop.clone().into();
            // If edge creates a neq cycle (a.k.a pres(edge.source) => edge.source != edge.source)
            // we can immediately deactivate it.
            if self.active_graph.creates_neq_cycle(edge) {
                model.set(
                    !prop.enabler.active,
                    self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
                )?;
            }
            // If propagator is active, we can propagate domains.
            if model.entails(enabler.active) {
                let res = self.propagate_bounds(model, edge);
                // if let Err(c) = res {}
                // Activate even if inconsistent so we can explain propagation later
                self.trail.push(Event::EdgeActivated(prop_id));
                self.active_graph.add_edge(edge);
                self.constraint_store.mark_active(prop_id);
                res?;
            }
        }
        Ok(())
    }

    fn propagate_eq(&self, model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomEq);
        let s_bounds = s.get_bounds(model);
        if let Node::Var(t) = t {
            model.set_lb(t, s_bounds.0, cause)?;
            model.set_ub(t, s_bounds.1, cause)?;
        } // else reverse propagator will be active, so nothing to do
          // TODO: Maybe handle reverse propagator immediately
        Ok(())
    }

    fn propagate_neq(&self, model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomNeq);
        // If domains don't overlap, nothing to do
        // If source domain is fixed and ub or lb of target == source lb, exclude that value
        debug_assert_ne!(s, t);

        if let Some(bound) = s.get_bound(model) {
            if let Node::Var(t) = t {
                if model.ub(t) == bound {
                    model.set_ub(t, bound - 1, cause)?;
                }
                if model.lb(t) == bound {
                    model.set_lb(t, bound + 1, cause)?;
                }
            }
        }
        Ok(())
    }

    /// Util closure used to filter edges that were not active at the time
    // TODO: Maybe also check is valid
    fn graph_filter_closure<'a>(model: &'a DomainsSnapshot<'a>) -> impl Fn(&Edge<Node, EdgeLabel>) -> bool + use<'a> {
        |e: &Edge<Node, EdgeLabel>| model.entails(e.label.l)
    }

    /// Explain a neq cycle inference as a path of edges.
    fn explain_neq_cycle_path(
        &self,
        propagator_id: PropagatorId,
        model: &DomainsSnapshot,
    ) -> Vec<Edge<Node, EdgeLabel>> {
        let prop = self.constraint_store.get_propagator(propagator_id);
        let edge: Edge<Node, EdgeLabel> = prop.clone().into();
        match prop.relation {
            EqRelation::Eq => self
                .active_graph
                .get_neq_path(edge.target, edge.source, Self::graph_filter_closure(model))
                .expect("Couldn't find explanation for cycle."),
            EqRelation::Neq => self
                .active_graph
                .get_eq_path(edge.target, edge.source, Self::graph_filter_closure(model))
                .expect("Couldn't find explanation for cycle."),
        }
    }

    /// Explain an equality inference as a path of edges.
    fn explain_eq_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<Edge<Node, EdgeLabel>> {
        let mut dft = self
            .active_graph
            .rev_eq_dft_path(Node::Var(literal.variable()), Self::graph_filter_closure(model));
        dft.next();
        dft.find(|(n, _)| {
            let (lb, ub) = n.get_bounds_snap(model);
            literal.svar().is_plus() && literal.variable().leq(ub).entails(literal)
                || literal.svar().is_minus() && literal.variable().geq(lb).entails(literal)
        })
        .map(|(n, _)| dft.get_path(n))
        .expect("Unable to explain eq propagation.")
    }

    /// Explain a neq inference as a path of edges.
    fn explain_neq_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<Edge<Node, EdgeLabel>> {
        let mut dft = self
            .active_graph
            .rev_eq_or_neq_dft_path(Node::Var(literal.variable()), Self::graph_filter_closure(model));
        dft.find(|(n, r)| {
            let (prev_lb, prev_ub) = model.bounds(literal.variable());
            // If relationship between node and literal node is Neq
            *r == EqRelation::Neq && {
                // If node is bound to a value
                if let Some(bound) = n.get_bound_snap(model) {
                    prev_ub == bound || prev_lb == bound
                } else {
                    false
                }
            }
        })
        .map(|(n, _)| dft.get_path(n))
        .expect("Unable to explain neq propagation.")
    }
}

impl Default for AltEqTheory {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for AltEqTheory {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|event| match event {
            Event::EdgeActivated(prop_id) => {
                self.active_graph
                    .remove_edge(self.constraint_store.get_propagator(prop_id).clone().into());
                self.constraint_store.mark_inactive(prop_id);
            }
        });
    }
}

impl Theory for AltEqTheory {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Eq(0)
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        debug_assert!(self.active_graph.iter_all_fwd().all(|e| model.entails(e.label.l)));
        self.stats.prop_count += 1;
        while let Some(event) = self.pending_activations.pop_front() {
            self.propagate_candidate(model, event.enabler, event.edge)?;
        }
        let mut x = 0;
        while let Some(event) = self.model_events.pop(model.trail()) {
            for (enabler, prop_id) in self
                .constraint_store
                .enabled_by(event.new_literal())
                .collect::<Vec<_>>() // To satisfy borrow checker
                .iter()
            {
                x += 1;
                self.propagate_candidate(model, *enabler, *prop_id)?;
            }
        }
        // if x != 0 {
        //     dbg!(x);
        // }
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
        let init_length = out_explanation.lits.len();
        self.stats.expl_count += 1;
        use ModelUpdateCause::*;

        // Get the path which explains the inference
        let cause = ModelUpdateCause::from(context.payload);
        let path = match cause {
            NeqCycle(prop_id) => self.explain_neq_cycle_path(prop_id, model),
            DomNeq => self.explain_neq_path(literal, model),
            DomEq => self.explain_eq_path(literal, model),
        };

        debug_assert!(path.iter().all(|e| model.entails(e.label.l)));
        out_explanation.extend(path.iter().map(|e| e.label.l));

        // Eq will also require the ub/lb of the literal which is at the "origin" of the propagation
        // (If the node is a varref)
        if cause == DomEq || cause == DomNeq {
            let origin = path
                .first()
                .expect("Node cannot be at the origin of it's own inference.")
                .target;
            if let Node::Var(v) = origin {
                if literal.svar().is_plus() || cause == DomNeq {
                    out_explanation.push(v.leq(model.ub(v)));
                }
                if literal.svar().is_minus() || cause == DomNeq {
                    out_explanation.push(v.geq(model.lb(v)));
                }
            }
        }

        // Neq will also require the previous ub/lb of itself
        if cause == DomNeq {
            let v = literal.variable();
            if literal.svar().is_plus() {
                out_explanation.push(v.leq(model.ub(v)));
            } else {
                out_explanation.push(v.geq(model.lb(v)));
            }
        }

        // Q: Do we need to add presence literals to the explanation?
        // A: Probably not
        self.stats.total_expl_length += out_explanation.lits.len() as u32 - init_length as u32;
    }

    fn print_stats(&self) {
        self.stats.print_stats();
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use core::panic;

    use hashbrown::HashSet;

    use crate::collections::seq::Seq;

    use super::*;

    fn test_with_backtrack<F>(mut f: F, eq: &mut AltEqTheory, model: &mut Domains)
    where
        F: FnMut(&mut AltEqTheory, &mut Domains),
    {
        eq.save_state();
        model.save_state();
        f(eq, model);
        eq.restore_last();
        model.restore_last();
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
        assert!(eq.propagate(&mut model).is_ok());
        assert_eq!(model.bounds(l.variable()), (0, 1));
        model.set(b_pres, Cause::Decision);
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

                eq.propagate(model);
                assert_eq!(model.lb(var4), 0);
            },
            &mut eq,
            &mut model,
        );

        test_with_backtrack(
            |eq, model| {
                model.set_lb(l2.variable(), 1, Cause::Decision).unwrap();

                eq.propagate(model);
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
    fn test_explain_neq() {
        let mut model = Domains::new();
        let mut eq = AltEqTheory::new();

        let a = model.new_var(0, 1);
        let b = model.new_var(0, 1);
        let c = model.new_var(0, 1);
        let l = model.new_var(0, 1).geq(1);
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

        model.decide(!l4);
        model.decide(l3);
        eq.propagate(&mut model);
        model.decide(a.geq(11));
        model.decide(!l2);
        model.decide(l1);

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
        eq.propagate(&mut model);
        assert_eq!(model.lb(var2), 1)
    }
}
