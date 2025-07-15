use itertools::Itertools;

use crate::{
    core::state::{Domains, InvalidUpdate},
    reasoners::{
        eq_alt::{
            graph::{DirEqGraph, Edge, NodePair},
            node::Node,
            propagators::{Enabler, PropagatorId},
            relation::EqRelation,
        },
        Contradiction,
    },
};

use super::{cause::ModelUpdateCause, AltEqTheory, Event};

impl AltEqTheory {
    /// Find some edge in the specified that forms a negative cycle with pair
    fn find_back_edge<'a>(&self, graph: &'a DirEqGraph<Node>, pair: &NodePair<Node>) -> Option<&'a Edge<Node>> {
        let NodePair {
            source,
            target,
            relation,
        } = *pair;
        graph
            .get_fwd_out_edges(target)?
            .iter()
            .find(|e| e.target == source && e.source == target && e.relation + relation == Some(EqRelation::Neq))
    }

    /// Propagate between pair.source and pair.target if edge were to be added
    fn propagate_pair(
        &self,
        model: &mut Domains,
        prop_id: PropagatorId,
        edge: Edge<Node>,
        pair: NodePair<Node>,
    ) -> Result<(), InvalidUpdate> {
        let NodePair {
            source,
            target,
            relation,
        } = pair;
        // Find an active edge which creates a negative cycle
        if self.find_back_edge(&self.active_graph, &pair).is_some() {
            model.set(
                !edge.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
            )?;
        }

        if model.entails(edge.active) {
            if let Some(back_edge) = self.find_back_edge(&self.undecided_graph, &pair) {
                model.set(
                    !back_edge.active,
                    self.identity.inference(ModelUpdateCause::NeqCycle(
                        self.constraint_store.get_id_from_edge(model, *back_edge),
                    )),
                )?;
            }
            match relation {
                EqRelation::Eq => {
                    self.propagate_eq(model, source, target)?;
                }
                EqRelation::Neq => {
                    self.propagate_neq(model, source, target)?;
                }
            };
        }

        Ok(())
    }

    /// Given an edge that is both active and valid but not added to the graph
    /// check all new paths a -=> b that will be created by this edge, and infer b's bounds from a
    fn propagate_edge(
        &mut self,
        model: &mut Domains,
        prop_id: PropagatorId,
        edge: Edge<Node>,
    ) -> Result<(), InvalidUpdate> {
        // Check for edge case
        if edge.source == edge.target && edge.relation == EqRelation::Neq {
            model.set(
                !edge.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
            )?;
        }
        // Get all new node pairs we can potentially propagate
        self.active_graph
            .paths_requiring(edge)
            .map(|p| -> Result<(), InvalidUpdate> { self.propagate_pair(model, prop_id, edge, p) })
            // Stop at first error
            .find(|x| x.is_err())
            .unwrap_or(Ok(()))
    }

    fn add_to_undecided_graph(&mut self, prop_id: PropagatorId, edge: Edge<Node>) {
        self.trail.push(Event::EdgeActivated(prop_id));
        if self.constraint_store.is_enabled(prop_id) {
            unreachable!();
            // self.active_graph.remove_edge(edge);
            // self.constraint_store.mark_inactive(prop_id);
        }
        self.undecided_graph.add_edge(edge);
        self.constraint_store.mark_inactive(prop_id);
    }

    fn add_to_active_graph(&mut self, prop_id: PropagatorId, edge: Edge<Node>) {
        self.trail.push(Event::EdgeActivated(prop_id));
        if self.undecided_graph.contains_edge(edge) {
            self.undecided_graph.remove_edge(edge);
        }
        self.active_graph.add_edge(edge);
        self.constraint_store.mark_active(prop_id);
    }

    /// Given any propagator, perform propagations if possible and necessary.
    pub fn propagate_candidate(
        &mut self,
        model: &mut Domains,
        enabler: Enabler,
        prop_id: PropagatorId,
    ) -> Result<(), Contradiction> {
        let prop = self.constraint_store.get_propagator(prop_id);
        let edge: Edge<Node> = prop.clone().into();
        // If not valid, nothing to do
        if !model.entails(enabler.valid) {
            return Ok(());
        }

        if !model.entails(enabler.active) && self.constraint_store.is_enabled(prop_id) {
            unreachable!();
            // self.active_graph.remove_edge(edge);
            // self.constraint_store.mark_inactive(prop_id);
            // return Ok(());
        }

        if model.entails(enabler.active) {
            let prop_res = self.propagate_edge(model, prop_id, edge);
            self.add_to_active_graph(prop_id, edge);
            prop_res?;
        } else if !model.entails(!enabler.active) {
            let prop_res = self.propagate_edge(model, prop_id, edge);
            self.add_to_undecided_graph(prop_id, edge);
            prop_res?;
        }
        Ok(())
    }

    fn propagate_eq(&self, model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomEq);
        let s_bounds = model.get_node_bounds(&s);
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

        if let Some(bound) = model.get_node_bound(&s) {
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
}
