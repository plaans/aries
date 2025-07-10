use crate::{
    core::state::{Domains, InvalidUpdate},
    reasoners::{
        eq_alt::{
            graph::Edge,
            node::Node,
            propagators::{Enabler, PropagatorId},
            relation::EqRelation,
        },
        Contradiction,
    },
};

use super::{cause::ModelUpdateCause, edge::EdgeLabel, AltEqTheory, Event};

impl AltEqTheory {
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
    pub fn propagate_candidate(
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
