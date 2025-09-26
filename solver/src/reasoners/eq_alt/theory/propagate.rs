use crate::{
    core::state::{Domains, InvalidUpdate},
    reasoners::{
        eq_alt::{
            constraints::ConstraintId,
            graph::Path,
            node::{Node, NodeDomains},
            relation::EqRelation,
        },
        Contradiction,
    },
};

use super::{cause::ModelUpdateCause, AltEqTheory};

impl AltEqTheory {
    // TODO: Shorten this function
    /// Propagate along `path` if constraint `constraint_id` were to be added to the graph.
    fn propagate_path(
        &mut self,
        model: &mut Domains,
        constraint_id: ConstraintId,
        path: Path,
    ) -> Result<(), InvalidUpdate> {
        let adding_constraint = self.constraint_store.get_constraint(constraint_id);

        // TODO: Evaluate if this can ever happen, my understanding is that
        // source -!=-> target can only happen if there is a constraint a != a
        if path.contradictory() {
            model.set(
                !adding_constraint.enabler.active,
                self.identity.inference(ModelUpdateCause::NeqCycle(constraint_id)),
            )?;
        }
        debug_assert!(!path.redundant());

        // Get the set of nodes that the new path comes from
        let source_nodes = self.enabled_graph.get_group_nodes(path.source_id);
        // Get the set of nodes that the new path leads to
        let target_nodes = self.enabled_graph.get_group_nodes(path.target_id);

        let constraints_into_source = source_nodes
            .iter()
            .flat_map(|n| self.constraint_store.get_in_constraints(*n));

        for in_constraint_id in constraints_into_source {
            let in_constraint = self.constraint_store.get_constraint(in_constraint_id);

            // Compose constraint relation with path relation
            let Some(total_relation) = in_constraint.relation + path.relation else {
                continue;
            };

            // If the constraint comes from the target with neq cycle => disable it
            if total_relation == EqRelation::Neq && target_nodes.contains(&in_constraint.a) {
                model.set(
                    !in_constraint.enabler.active,
                    self.identity.inference(ModelUpdateCause::NeqCycle(in_constraint_id)),
                )?;
            }

            // If the constraint's source node bounds don't match the target, disable it
            if match total_relation {
                EqRelation::Eq => !in_constraint.a.can_be_eq(&target_nodes[0], model),
                EqRelation::Neq => !in_constraint.a.can_be_neq(&target_nodes[0], model),
            } && model.entails(in_constraint.enabler.valid)
            {
                model.set(
                    !in_constraint.enabler.active,
                    self.identity
                        .inference(ModelUpdateCause::EdgeDeactivation(in_constraint_id, true)),
                )?;
            }
        }

        let constraints_out_target = target_nodes
            .iter()
            .flat_map(|n| self.constraint_store.get_out_constraints(*n));

        for out_constraint_id in constraints_out_target {
            let out_constraint = self.constraint_store.get_constraint(out_constraint_id);

            let Some(total_relation) = out_constraint.relation + path.relation else {
                continue;
            };

            if match total_relation {
                EqRelation::Eq => !source_nodes[0].can_be_eq(&out_constraint.b, model),
                EqRelation::Neq => !source_nodes[0].can_be_neq(&out_constraint.b, model),
            } && model.entails(out_constraint.enabler.valid)
            {
                model.set(
                    !out_constraint.enabler.active,
                    self.identity
                        .inference(ModelUpdateCause::EdgeDeactivation(out_constraint_id, false)),
                )?;
            }
        }

        // Propagate eq and neq between all members of affected groups
        // All members of group should have same domains, so we can prop from one source to all targets
        let source = self.enabled_graph.get_node(path.source_id.into());
        match path.relation {
            EqRelation::Eq => {
                for target in target_nodes {
                    self.propagate_eq(model, source, target)?;
                }
            }
            EqRelation::Neq => {
                for target in target_nodes {
                    self.propagate_neq(model, source, target)?;
                }
            }
        };

        Ok(())
    }

    /// Given a constraint that has just been enabled, run propagations on all new paths it creates.
    pub fn propagate_edge(&mut self, model: &mut Domains, constraint_id: ConstraintId) -> Result<(), Contradiction> {
        let constraint = self.constraint_store.get_constraint(constraint_id);

        debug_assert!(model.entails(constraint.enabler.active));
        debug_assert!(model.entails(constraint.enabler.valid));

        let edge = self.enabled_graph.create_edge(constraint);

        // Check for edge case
        if edge.source == edge.target && edge.relation == EqRelation::Neq {
            return Err(model
                .set(
                    !edge.active,
                    self.identity.inference(ModelUpdateCause::NeqCycle(constraint_id)),
                )
                .unwrap_err()
                .into());
        }

        // Get all new paths we can potentially propagate along
        let paths = self.enabled_graph.paths_requiring(edge);

        self.stats().total_paths += paths.len() as u32;
        self.stats().edges_propagated += 1;

        if paths.is_empty() {
            // Edge is redundant, don't add it to the graph
            return Ok(());
        } else {
            debug_assert!(!self
                .enabled_graph
                .outgoing_grouped
                .iter_edges(edge.source)
                .any(|e| e.target == edge.target && e.relation == edge.relation));
        }

        let res = paths
            .into_iter()
            .try_for_each(|p| self.propagate_path(model, constraint_id, p));

        // If we have a <=> b, we can merge a and b together
        // For now, only handle the simplest case of Eq fusion, a -=-> b && b -=-> a
        // Theoretically, this should be sufficient, as implication cycles should automatically go both ways
        // However due to limits in the implication graph, this is not sufficient, but good enough
        if edge.relation == EqRelation::Eq
            && self
                .enabled_graph
                .outgoing_grouped
                .iter_edges(edge.target)
                .any(|e| e.target == edge.source && e.relation == EqRelation::Eq)
        {
            self.stats().merges += 1;
            self.enabled_graph.merge((edge.source, edge.target));
        }

        // Once all propagations are complete, we can add edge to the graph
        self.enabled_graph.add_edge(edge);
        Ok(res?)
    }

    /// Propagate `target`'s bounds where `source` -=-> `target`
    ///
    /// dom(target) := dom(target) U dom(source)
    fn propagate_eq(&self, model: &mut Domains, source: Node, target: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomEq);
        let s_bounds = model.node_domain(&source);
        if let Node::Var(t) = target {
            if model.set_lb(t, s_bounds.lb, cause)? {
                self.stats().eq_props += 1;
            }
            if model.set_ub(t, s_bounds.ub, cause)? {
                self.stats().eq_props += 1;
            }
        } // else reverse constraint will be active, so nothing to do
          // TODO: Maybe handle reverse constraint immediately
        Ok(())
    }

    /// Propagate `target`'s bounds where `source` -!=-> `target`
    /// dom(target) := dom(target) \ dom(source) if |dom(source)| = 1
    fn propagate_neq(&self, model: &mut Domains, source: Node, target: Node) -> Result<(), InvalidUpdate> {
        let cause = self.identity.inference(ModelUpdateCause::DomNeq);
        // If domains don't overlap, nothing to do
        // If source domain is fixed and ub or lb of target == source lb, exclude that value
        debug_assert_ne!(source, target);

        if let Some(bound) = model.node_domain(&source).as_singleton() {
            if let Node::Var(t) = target {
                if model.ub(t) == bound && model.set_ub(t, bound - 1, cause)? {
                    self.stats().neq_props += 1;
                }
                if model.lb(t) == bound && model.set_lb(t, bound + 1, cause)? {
                    self.stats().neq_props += 1;
                }
            }
        }
        Ok(())
    }
}
