use crate::{
    core::state::{Domains, InvalidUpdate},
    reasoners::{
        eq_alt::{
            graph::{GraphDir, IdEdge, Path},
            node::Node,
            propagators::PropagatorId,
            relation::EqRelation,
        },
        Contradiction,
    },
};

use super::{cause::ModelUpdateCause, AltEqTheory};

impl AltEqTheory {
    /// Propagate along `path` if `edge` (identified by `prop_id`) were to be added to the graph
    fn propagate_path(
        &mut self,
        model: &mut Domains,
        prop_id: PropagatorId,
        edge: IdEdge,
        path: Path,
    ) -> Result<(), InvalidUpdate> {
        let prop = self.constraint_store.get_propagator(prop_id);
        let Path {
            source_id,
            target_id,
            relation,
        } = path;
        if source_id == target_id {
            match relation {
                EqRelation::Neq => {
                    model.set(
                        !prop.enabler.active,
                        self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
                    )?;
                }
                EqRelation::Eq => {
                    // Not sure if we should handle cycles here, quite inconsistent
                    // Works for triangles but not pairs
                    return Ok(());
                }
            }
        }
        debug_assert!(model.entails(edge.active));

        // Find propagators which create a negative cycle, then disable them
        self.active_graph
            .group_product(path.source_id, path.target_id)
            .flat_map(|(source, target)| self.constraint_store.get_from_nodes(target, source))
            .filter_map(|id| {
                let prop = self.constraint_store.get_propagator(id);
                (path.relation + prop.relation == Some(EqRelation::Neq)).then_some((id, prop.clone()))
            })
            .try_for_each(|(id, prop)| {
                self.stats.neq_cycle_props += 1;
                model
                    .set(
                        !prop.enabler.active,
                        self.identity.inference(ModelUpdateCause::NeqCycle(id)),
                    )
                    .map(|_| ())
            })?;

        // Propagate eq and neq between all members of affected groups
        // All members of group should have same domains, so we can prop from one source to all targets
        let source = self.active_graph.get_node(source_id.into());
        match relation {
            EqRelation::Eq => {
                for target in self.active_graph.get_group_nodes(target_id) {
                    self.propagate_eq(model, source, target)?;
                }
            }
            EqRelation::Neq => {
                for target in self.active_graph.get_group_nodes(target_id) {
                    self.propagate_neq(model, source, target)?;
                }
            }
        };

        Ok(())
    }

    /// Given any propagator, perform propagations if possible and necessary.
    pub fn propagate_edge(&mut self, model: &mut Domains, prop_id: PropagatorId) -> Result<(), Contradiction> {
        let prop = self.constraint_store.get_propagator(prop_id);

        debug_assert!(model.entails(prop.enabler.active));
        debug_assert!(model.entails(prop.enabler.valid));

        let edge = self.active_graph.create_edge(prop);

        // Check for edge case
        if edge.source == edge.target && edge.relation == EqRelation::Neq {
            model.set(
                !edge.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
            )?;
            return Ok(());
        }

        // Get all new node paths we can potentially propagate along
        let paths = self.active_graph.paths_requiring(edge);
        self.stats.total_paths += paths.len() as u32;
        self.stats.edges_propagated += 1;
        if paths.is_empty() {
            // Edge is redundant, don't add it to the graph
            return Ok(());
        } else {
            debug_assert!(!self
                .active_graph
                .get_out_edges(edge.source, GraphDir::ForwardGrouped)
                .iter()
                .any(|e| e.target == edge.target && e.relation == edge.relation));
        }

        let res = paths
            .into_iter()
            .try_for_each(|p| self.propagate_path(model, prop_id, edge, p));

        // For now, only handle the simplest case of Eq fusion, a -=-> b && b -=-> a
        // Theoretically, this should be sufficient, as implication cycles should automatically go both ways
        // However to due limits in the implication graph, this is not sufficient, but good enough
        if edge.relation == EqRelation::Eq
            && self
                .active_graph
                .get_out_edges(edge.target, GraphDir::ForwardGrouped)
                .into_iter()
                .any(|e| e.target == edge.source && e.relation == EqRelation::Eq)
        {
            self.stats.merges += 1;
            self.active_graph.merge((edge.source, edge.target));
        }

        self.active_graph.add_edge(edge);
        Ok(res?)
    }

    /// Propagate `s` and `t`'s bounds if s -=-> t
    fn propagate_eq(&mut self, model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomEq);
        let s_bounds = model.node_bounds(&s);
        if let Node::Var(t) = t {
            if model.set_lb(t, s_bounds.0, cause)? {
                self.stats.eq_props += 1;
            }
            if model.set_ub(t, s_bounds.1, cause)? {
                self.stats.eq_props += 1;
            }
        } // else reverse propagator will be active, so nothing to do
          // TODO: Maybe handle reverse propagator immediately
        Ok(())
    }

    /// Propagate `s` and `t`'s bounds if s -!=-> t
    fn propagate_neq(&mut self, model: &mut Domains, s: Node, t: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomNeq);
        // If domains don't overlap, nothing to do
        // If source domain is fixed and ub or lb of target == source lb, exclude that value
        debug_assert_ne!(s, t);

        if let Some(bound) = model.node_bound(&s) {
            if let Node::Var(t) = t {
                if model.ub(t) == bound && model.set_ub(t, bound - 1, cause)? {
                    self.stats.neq_props += 1;
                }
                if model.lb(t) == bound && model.set_lb(t, bound + 1, cause)? {
                    self.stats.neq_props += 1;
                }
            }
        }
        Ok(())
    }
}
