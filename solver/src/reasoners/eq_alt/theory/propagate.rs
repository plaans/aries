use crate::{
    core::state::{Domains, InvalidUpdate},
    reasoners::{
        eq_alt::{
            graph::{Edge, NodePair},
            node::Node,
            propagators::{Enabler, Propagator, PropagatorId},
            relation::EqRelation,
        },
        Contradiction,
    },
};

use super::{cause::ModelUpdateCause, AltEqTheory, Event};

impl AltEqTheory {
    /// Find some edge in the specified that forms a negative cycle with pair
    fn find_back_edge(
        &self,
        model: &Domains,
        active: bool,
        pair: &NodePair<Node>,
    ) -> Option<(PropagatorId, Propagator)> {
        let NodePair {
            source,
            target,
            relation,
        } = *pair;
        self.constraint_store
            .get_from_nodes(pair.target, pair.source)
            .iter()
            .find_map(|id| {
                let prop = self.constraint_store.get_propagator(*id);
                assert!(model.entails(prop.enabler.valid));
                let activity_ok = active && self.constraint_store.marked_active(id)
                    || !active && !model.entails(prop.enabler.active) && !model.entails(!prop.enabler.active);
                (activity_ok
                    && prop.a == target
                    && prop.b == source
                    && relation + prop.relation == Some(EqRelation::Neq))
                .then_some((*id, prop.clone()))
            })
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
        if let Some((_id, _back_prop)) = self.find_back_edge(model, true, &pair) {
            model.set(
                !edge.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
            )?;
        }

        if model.entails(edge.active) {
            if let Some((id, back_prop)) = self.find_back_edge(model, false, &pair) {
                // println!("back edge: {back_prop:?}");
                model.set(
                    !back_prop.enabler.active,
                    self.identity.inference(ModelUpdateCause::NeqCycle(id)),
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

    /// Given any propagator, perform propagations if possible and necessary.
    pub fn propagate_candidate(
        &mut self,
        model: &mut Domains,
        enabler: Enabler,
        prop_id: PropagatorId,
    ) -> Result<(), Contradiction> {
        let prop = self.constraint_store.get_propagator(prop_id);
        let edge: Edge<Node> = prop.clone().into();
        // If not valid or inactive, nothing to do
        if !model.entails(enabler.valid) || model.entails(!enabler.active) {
            return Ok(());
        }

        // If propagator is newly activated, propagate and add
        if model.entails(enabler.active) && !self.constraint_store.marked_active(&prop_id) {
            let res = self.propagate_edge(model, prop_id, edge);
            // If the propagator was previously undecided, we know it was just activated
            self.trail.push(Event::EdgeActivated(prop_id));
            self.active_graph.add_edge(edge);
            self.constraint_store.mark_active(prop_id);
            res?;
        } else if !model.entails(enabler.active) {
            self.propagate_edge(model, prop_id, edge)?;
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
