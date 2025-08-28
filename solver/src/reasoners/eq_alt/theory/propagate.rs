use itertools::Itertools;

use crate::{
    core::state::{Domains, InvalidUpdate},
    reasoners::{
        eq_alt::{
            graph::{folds::EqFold, traversal::GraphTraversal, GraphDir, IdEdge, Path, TaggedNode},
            node::Node,
            propagators::{Propagator, PropagatorId},
            relation::EqRelation,
        },
        Contradiction,
    },
};

use super::{cause::ModelUpdateCause, AltEqTheory};

impl AltEqTheory {
    /// Merge all nodes in a cycle together.
    fn merge_cycle(&mut self, path: &Path, edge: IdEdge) {
        let g = &self.active_graph;
        let path_source = g.get_group_id(path.source_id.into());
        let path_target = g.get_group_id(path.target_id.into());
        let edge_source = g.get_group_id(edge.source).into();
        let edge_target = g.get_group_id(edge.target).into();

        // Get path from path.source to edge.source and the path from edge.target to path.target
        let source_path = {
            let mut traversal = GraphTraversal::new(
                self.active_graph.get_traversal_graph(GraphDir::ForwardGrouped),
                EqFold(),
                path_source.into(),
                true,
            );
            let n = traversal.find(|&TaggedNode(n, ..)| n == edge_source).unwrap();
            traversal.get_path(n)
        };

        let target_path = {
            let mut traversal = GraphTraversal::new(
                self.active_graph.get_traversal_graph(GraphDir::ForwardGrouped),
                EqFold(),
                edge_target,
                true,
            );
            let n = traversal.find(|&TaggedNode(n, ..)| n == path_target.into()).unwrap();
            traversal.get_path(n)
        };

        self.active_graph.merge((edge_target, edge_source));
        for edge in source_path.into_iter().chain(target_path) {
            self.active_graph.merge((edge.target, path_source.into()));
        }
    }

    /// Find an edge which completes a cycle when added to the path pair
    ///
    /// Optionally returns an edge from pair.target to pair.source such that pair.relation + edge.relation = check_relation
    /// * `active`: If true, the edge must be marked as active (present in active graph), else it's activity must be undecided according to the model
    fn find_back_edge(
        &self,
        model: &Domains,
        active: bool,
        path: &Path,
        check_relation: EqRelation,
    ) -> Option<(PropagatorId, Propagator)> {
        let sources = self.active_graph.get_group_nodes(path.source_id);
        let targets = self.active_graph.get_group_nodes(path.target_id);

        sources
            .into_iter()
            .cartesian_product(targets)
            .find_map(|(source, target)| {
                self.constraint_store
                    .get_from_nodes(target, source)
                    .iter()
                    .find_map(|id| {
                        let prop = self.constraint_store.get_propagator(*id);
                        assert!(model.entails(prop.enabler.valid));
                        let activity_ok = active && self.constraint_store.marked_active(id)
                            || !active && !model.entails(prop.enabler.active) && !model.entails(!prop.enabler.active);
                        (activity_ok
                            && prop.a == target
                            && prop.b == source
                            && path.relation + prop.relation == Some(check_relation))
                        .then_some((*id, prop.clone()))
                    })
            })
    }

    /// Propagate along `path` if `edge` (identified by `prop_id`) were to be added to the graph
    fn propagate_path(
        &mut self,
        model: &mut Domains,
        prop_id: PropagatorId,
        edge: IdEdge,
        path: Path,
    ) -> Result<(), InvalidUpdate> {
        let Path {
            source_id,
            target_id,
            relation,
        } = path;

        // Find an active edge which creates a negative cycle, then disable current edge
        if let Some((_id, _back_prop)) = self.find_back_edge(model, true, &path, EqRelation::Neq) {
            self.stats.neq_cycle_props += 1;
            model.set(
                !edge.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
            )?;
        }

        if model.entails(edge.active) {
            // Find some activity undecided edge which creates a negative cycle, then disable it
            if let Some((id, back_prop)) = self.find_back_edge(model, false, &path, EqRelation::Neq) {
                self.stats.neq_cycle_props += 1;
                model.set(
                    !back_prop.enabler.active,
                    self.identity.inference(ModelUpdateCause::NeqCycle(id)),
                )?;
            }

            // Propagate eq and neq between all members of affected groups
            let sources = self.active_graph.node_store.get_group_nodes(source_id);
            let targets = self.active_graph.node_store.get_group_nodes(target_id);

            match relation {
                EqRelation::Eq => {
                    for (source, target) in sources.into_iter().cartesian_product(targets) {
                        self.propagate_eq(model, source, target)?;
                    }
                }
                EqRelation::Neq => {
                    for (source, target) in sources.into_iter().cartesian_product(targets) {
                        self.propagate_neq(model, source, target)?;
                    }
                }
            };

            // If we detect an eq cycle, find the path that created this cycle and merge
            if self.find_back_edge(model, true, &path, EqRelation::Eq).is_some() {
                self.stats.merges += 1;
                self.merge_cycle(&path, edge);
            }
        }

        Ok(())
    }

    /// Propagate if `edge` were to be added to the graph
    fn propagate_edge(
        &mut self,
        model: &mut Domains,
        prop_id: PropagatorId,
        edge: IdEdge,
    ) -> Result<(), InvalidUpdate> {
        // Check for edge case
        if edge.source == edge.target && edge.relation == EqRelation::Neq {
            model.set(
                !edge.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(prop_id)),
            )?;
        }

        // Get all new node paths we can potentially propagate
        let paths = self.active_graph.paths_requiring(edge);
        self.stats.total_paths += paths.len() as u32;
        self.stats.edges_propagated += 1;

        paths
            .into_iter()
            .try_for_each(|p| self.propagate_path(model, prop_id, edge, p))
    }

    /// Given any propagator, perform propagations if possible and necessary.
    pub fn propagate_candidate(&mut self, model: &mut Domains, prop_id: PropagatorId) -> Result<(), Contradiction> {
        let prop = self.constraint_store.get_propagator(prop_id);
        let edge = self.active_graph.create_edge(prop);
        // If not valid or inactive, nothing to do
        if !model.entails(prop.enabler.valid) || model.entails(!prop.enabler.active) {
            return Ok(());
        }

        // If propagator is newly activated, propagate and add
        if model.entails(prop.enabler.active) && !self.constraint_store.marked_active(&prop_id) {
            let res = self.propagate_edge(model, prop_id, edge);
            // If the propagator was previously undecided, we know it was just activated
            self.active_graph.add_edge(edge);
            self.constraint_store.mark_active(prop_id);
            res?;
        } else if !model.entails(prop.enabler.active) {
            self.propagate_edge(model, prop_id, edge)?;
        }

        Ok(())
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
