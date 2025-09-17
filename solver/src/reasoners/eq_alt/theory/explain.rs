use itertools::Itertools;

use crate::{
    core::{
        state::{DomainsSnapshot, Explanation},
        Lit,
    },
    reasoners::eq_alt::{
        constraints::ConstraintId,
        graph::{
            transforms::{EqExt, EqNeqExt, EqNode, FilterExt},
            traversal::{Graph, PathStore},
            Edge, NodeId,
        },
        node::Node,
        relation::EqRelation,
        theory::cause::ModelUpdateCause,
    },
};

use super::AltEqTheory;

impl AltEqTheory {
    /// Get the path of enabled edges from prop.target to prop.source.
    /// This should allow us to explain a cycle propagation.
    pub fn neq_cycle_explanation_path(&self, constraint_id: ConstraintId, model: &DomainsSnapshot) -> Vec<Edge> {
        let constraint = self.constraint_store.get_constraint(constraint_id);
        let source_id = self.active_graph.get_id(&constraint.b).unwrap();
        let target_id = self.active_graph.get_id(&constraint.a).unwrap();

        // Transform the enabled graph to get a snapshot of it just before the propagation
        let graph = self.active_graph.outgoing.filter(|_, e| model.entails(e.active));

        match constraint.relation {
            EqRelation::Eq => {
                let mut path_store = PathStore::new();
                // Find a path from target to source with relation Neq
                graph
                    .eq_neq()
                    .traverse_bfs(EqNode::new(source_id), &mut Default::default())
                    .record_paths(&mut path_store)
                    .find(|&n| n == EqNode(target_id, EqRelation::Neq))
                    .map(|n| path_store.get_path(n).map(|e| e.0).collect_vec())
            }
            EqRelation::Neq => {
                let mut path_store = PathStore::new();
                // Find a path from target to source with relation Eq
                graph
                    .eq()
                    .traverse_bfs(source_id, &mut Default::default())
                    .record_paths(&mut path_store)
                    .find(|&n| n == target_id)
                    .map(|n| path_store.get_path(n).collect_vec())
            }
        }
        .unwrap_or_else(|| {
            let a_id = self.active_graph.get_id(&constraint.a).unwrap();
            let b_id = self.active_graph.get_id(&constraint.b).unwrap();
            panic!(
                "Unable to explain active graph: \n\
                    {}\n\
                    {}\n\
                    {:?}\n\
                    ({:?} -{}-> {:?}),\n\
                    ({:?} -{}-> {:?})",
                self.active_graph.to_graphviz(),
                self.active_graph.to_graphviz_grouped(),
                constraint,
                a_id,
                constraint.relation,
                b_id,
                self.active_graph.get_group_id(a_id),
                constraint.relation,
                self.active_graph.get_group_id(b_id)
            )
        })
    }

    /// Look for a path from the variable whose bounds were modified to any variable which
    /// could have caused the bound update though equality propagation.
    pub fn eq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<Edge> {
        let source_id = self.active_graph.get_id(&Node::Var(literal.variable())).unwrap();

        let mut path_store = PathStore::new();
        let cause = self
            .active_graph
            .incoming
            .filter(|_, e| model.entails(e.active))
            .eq()
            .traverse_bfs(source_id, &mut Default::default())
            .record_paths(&mut path_store)
            .skip(1) // Cannot cause own propagation
            .find(|id| self.can_explain_eq(literal, *id, model))
            .expect("Unable to explain eq propagation");
        path_store.get_path(cause).collect()
    }

    /// Look for a path from the variable whose bounds were modified to any variable which
    /// could have caused the bound update though inequality propagation.
    pub fn neq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<Edge> {
        let source_id = self.active_graph.get_id(&Node::Var(literal.variable())).unwrap();

        let mut path_store = PathStore::new();
        let cause = self
            .active_graph
            .incoming
            .filter(|_, e| model.entails(e.active))
            .eq_neq()
            .traverse_bfs(EqNode::new(source_id), &mut Default::default())
            .record_paths(&mut path_store)
            .skip(1)
            .find(|EqNode(id, r)| *r == EqRelation::Neq && self.can_explain_neq(literal, *id, model))
            .expect("Unable to explain Neq propagation");

        path_store.get_path(cause).map(|e| e.0).collect()
    }

    fn can_explain_eq(&self, literal: Lit, potential_cause: NodeId, model: &DomainsSnapshot<'_>) -> bool {
        let n = self.active_graph.get_node(potential_cause);

        let node_domain = model.node_domain(&n);
        node_domain.entails(literal)
    }

    fn can_explain_neq(&self, literal: Lit, potential_cause: NodeId, model: &DomainsSnapshot<'_>) -> bool {
        let (prev_lb, prev_ub) = model.bounds(literal.variable());
        // If relationship between node and literal node is Neq
        let n = self.active_graph.get_node(potential_cause);
        // If node is bound to a value
        if let Some(bound) = model.node_domain(&n).as_singleton() {
            prev_ub == bound || prev_lb == bound
        } else {
            false
        }
    }

    /// Given a path computed from one of the functions defined above, constructs an explanation from this path
    pub fn explain_from_path(
        &self,
        model: &DomainsSnapshot<'_>,
        literal: Lit,
        cause: ModelUpdateCause,
        path: Vec<Edge>,
        out_explanation: &mut Explanation,
    ) {
        use ModelUpdateCause::*;
        out_explanation.extend(path.iter().map(|e| e.active));

        // Eq will also require the ub/lb of the literal which is at the "origin" of the propagation
        // (If the node is a varref)
        if cause == DomEq || cause == DomNeq {
            let origin = self.active_graph.get_node(
                path.first()
                    .expect("Node cannot be at the origin of it's own inference.")
                    .target,
            );
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
    }
}
