#![allow(unused)]

use std::collections::VecDeque;

use hashbrown::HashMap;
use tracing::event;

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail},
    core::{
        state::{Cause, Domains, DomainsSnapshot, Explanation, InferenceCause, InvalidUpdate, Term},
        IntCst, Lit, Relation, VarRef,
    },
    reasoners::{
        eq_alt::{
            core::{EqRelation, Node},
            graph::{DirEqGraph, Edge, NodePair},
            propagators::{Enabler, Propagator, PropagatorId, PropagatorStore},
        },
        Contradiction, ReasonerId, Theory,
    },
};

use super::propagators::ActivationEvent;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
struct EdgeLabel {
    l: Lit,
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

#[derive(Clone)]
pub struct AltEqTheory {
    constraint_store: PropagatorStore,
    active_graph: DirEqGraph<Node, EdgeLabel>,
    model_events: ObsTrailCursor<ModelEvent>,
    pending_activations: VecDeque<ActivationEvent>,
    trail: Trail<Event>,
}

impl AltEqTheory {
    pub fn new() -> Self {
        AltEqTheory {
            constraint_store: Default::default(),
            active_graph: DirEqGraph::new(),
            model_events: Default::default(),
            trail: Default::default(),
            pending_activations: Default::default(),
        }
    }

    /// Add l => (a = b) constraint. a must be a variable, but b can also be a constant
    pub fn add_half_reified_eq_edge(&mut self, l: Lit, a: VarRef, b: impl Into<Node>, model: &Domains) {
        self.add_edge(l, a, b, EqRelation::Eq, model);
    }

    /// Add l => (a != b) constraint
    pub fn add_half_reified_neq_edge(&mut self, l: Lit, a: VarRef, b: impl Into<Node>, model: &Domains) {
        self.add_edge(l, a, b, EqRelation::Neq, model);
    }

    fn add_edge(&mut self, l: Lit, a: VarRef, b: impl Into<Node>, relation: EqRelation, model: &Domains) {
        let b = b.into();
        let pa = model.presence(a);
        let pb = model.presence(b);

        // When pb => pa, edge a -> b is always valid
        let ab_valid = if model.implies(pb, pa) { Lit::TRUE } else { pa };
        let ba_valid = if model.implies(pa, pb) { Lit::TRUE } else { pb };

        let (ab_prop, ba_prop) = Propagator::new_pair(a.into(), b, relation, l, ab_valid, ba_valid);
        let ab_enabler = ab_prop.enabler;
        let ba_enabler = ba_prop.enabler;
        let ab_id = self.constraint_store.add_propagator(ab_prop);
        let ba_id = self.constraint_store.add_propagator(ba_prop);
        self.active_graph.add_node(a.into());
        self.active_graph.add_node(b);
        if model.entails(ab_valid) && model.entails(l) {
            self.pending_activations
                .push_back(ActivationEvent::new(ab_id, ab_enabler));
        }
        if model.entails(ba_valid) && model.entails(l) {
            self.pending_activations
                .push_back(ActivationEvent::new(ba_id, ba_enabler));
        }
    }

    fn activate_propagator(&mut self, model: &mut Domains, prop_id: PropagatorId) -> Result<(), Contradiction> {
        let prop = self.constraint_store.get_propagator(prop_id);
        let edge = prop.clone().into();
        if let Some(e) = self
            .active_graph
            .paths_requiring(edge)
            .map(|p| -> Result<(), InvalidUpdate> {
                match p.relation {
                    EqRelation::Eq => {
                        propagate_eq(model, p.source, p.target)?;
                        if self.active_graph.neq_path_exists(p.source, p.target) {
                            model.set(
                                !prop.enabler.active,
                                Cause::Inference(InferenceCause {
                                    writer: ReasonerId::Eq(0),
                                    payload: 0,
                                }),
                            )?;
                        }
                    }
                    EqRelation::Neq => {
                        propagate_neq(model, p.source, p.target)?;
                        if self.active_graph.eq_path_exists(p.source, p.target) {
                            model.set(
                                !prop.enabler.active,
                                Cause::Inference(InferenceCause {
                                    writer: ReasonerId::Eq(0),
                                    payload: 0,
                                }),
                            )?;
                        }
                    }
                };
                Ok(())
            })
            .find(|x| x.is_err())
        {
            e?
        };
        self.trail.push(Event::EdgeActivated(prop_id));
        self.active_graph.add_edge(edge);
        self.constraint_store.mark_active(prop_id);
        Ok(())
    }

    fn propagate_candidates<'a>(
        &mut self,
        model: &mut Domains,
        enable_candidates: impl Iterator<Item = &'a (Enabler, PropagatorId)>,
    ) -> Result<(), Contradiction> {
        let to_enable = enable_candidates
            .filter(|(enabler, prop_id)| {
                model.entails(enabler.active)
                    && model.entails(enabler.valid)
                    && !self.constraint_store.is_active(*prop_id)
            })
            .collect::<Vec<_>>();
        Ok(
            if let Some(err) = to_enable
                .iter()
                .map(|(enabler, prop_id)| self.activate_propagator(model, *prop_id))
                .find(|r| r.is_err())
            {
                err?
            },
        )
    }
}

impl Backtrack for AltEqTheory {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state();
        todo!()
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
        let mut new_activations = vec![];
        while let Some(event) = self.pending_activations.pop_front() {
            new_activations.push((event.enabler, event.edge));
        }
        self.propagate_candidates(model, new_activations.iter())?;

        while let Some(event) = self.model_events.pop(model.trail()) {
            let enable_candidates: Vec<_> = self.constraint_store.enabled_by(event.new_literal()).collect();
            // Vec of all propagators which are newly enabled by this event
            self.propagate_candidates(model, enable_candidates.iter())?;
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
        todo!()
    }

    fn print_stats(&self) {
        todo!()
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

fn propagate_eq(model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
    let cause = Cause::Inference(InferenceCause {
        writer: ReasonerId::Eq(0),
        payload: 0,
    });
    let s_bounds = match s {
        Node::Var(v) => (model.lb(v), model.ub(v)),
        Node::Val(v) => (v, v),
    };
    if let Node::Var(t) = t {
        model.set_lb(t, s_bounds.0, cause)?;
        model.set_ub(t, s_bounds.1, cause)?;
    } // else reverse propagator will be active, so nothing to do
    Ok(())
}

fn propagate_neq(model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
    let cause = Cause::Inference(InferenceCause {
        writer: ReasonerId::Eq(0),
        payload: 0,
    });
    // If domains don't overlap, nothing to do
    // If source domain is fixed and ub or lb of target == source lb, exclude that value
    let (s_lb, s_ub) = match s {
        Node::Var(v) => (model.lb(v), model.ub(v)),
        Node::Val(v) => (v, v),
    };
    if let Node::Var(t) = t {
        if s_lb == s_ub {
            if model.ub(t) == s_lb {
                model.set_ub(t, s_lb - 1, cause)?;
            }
            if model.lb(t) == s_lb {
                model.set_lb(t, s_lb + 1, cause)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

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

        eq.add_half_reified_eq_edge(l2, var3, var4, &model);
        eq.add_half_reified_eq_edge(l2, var4, var5, &model);
        eq.add_half_reified_eq_edge(l2, var3, 1 as IntCst, &model);

        eq.propagate(&mut model);
        assert_eq!(model.lb(var4), 0);

        model.set_lb(l2.variable(), 1, Cause::Decision).unwrap();

        eq.propagate(&mut model);
        assert_eq!(model.lb(var4), 1);
        assert_eq!(model.lb(var5), 1);
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

        eq.add_half_reified_eq_edge(l2, var3, var4, &model);
        eq.add_half_reified_neq_edge(l2, var3, var5, &model);
        eq.add_half_reified_eq_edge(l2, var4, var5, &model);
        // eq.add_half_reified_eq_edge(l2, var3, 1 as IntCst, &model);

        model.set_lb(l2.variable(), 1, Cause::Decision).unwrap();
        eq.propagate(&mut model).expect_err("Contradiction.");
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

        eq.add_half_reified_eq_edge(l, a, b, &model);
        eq.add_half_reified_eq_edge(l, b, c, &model);
        eq.add_half_reified_eq_edge(l, c, 1 as IntCst, &model);

        eq.propagate(&mut model).unwrap();

        assert_eq!(model.lb(c), 1);
        assert_eq!(model.lb(b), 0);
        assert_eq!(model.lb(a), 0);

        eq.add_half_reified_eq_edge(l, a, 1 as IntCst, &model);
        eq.propagate(&mut model).unwrap();

        assert_eq!(model.lb(c), 1);
        assert_eq!(model.lb(b), 1);
        assert_eq!(model.lb(a), 1);
    }

    #[test]
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

        eq.add_half_reified_eq_edge(l, a, b, &model);
        eq.add_half_reified_eq_edge(l, b, c, &model);
        eq.add_half_reified_neq_edge(l, a, c, &model);

        eq.propagate(&mut model).expect_err("Contradiction.");
    }
}
