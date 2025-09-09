use itertools::Itertools;

use crate::{
    core::{
        state::{DomainsSnapshot, Explanation},
        Lit,
    },
    reasoners::eq_alt::{
        graph::{
            transforms::{EqExt, EqNeqExt, EqNode, FilterExt},
            traversal::{Graph, PathStore},
            IdEdge,
        },
        node::Node,
        propagators::PropagatorId,
        relation::EqRelation,
        theory::cause::ModelUpdateCause,
    },
};

use super::AltEqTheory;

impl AltEqTheory {
    /// Explain a neq cycle inference as a path of edges.
    pub fn neq_cycle_explanation_path(&self, prop_id: PropagatorId, model: &DomainsSnapshot) -> Vec<IdEdge> {
        let prop = self.constraint_store.get_propagator(prop_id);
        let source_id = self.active_graph.get_id(&prop.b).unwrap();
        let target_id = self.active_graph.get_id(&prop.a).unwrap();

        let graph = self.active_graph.outgoing.filter(|_, e| model.entails(e.active));

        match prop.relation {
            EqRelation::Eq => {
                let mut path_store = PathStore::new();
                graph
                    .eq_neq()
                    .traverse(EqNode::new(source_id), &mut Default::default())
                    .mem_path(&mut path_store)
                    .find(|&n| n == EqNode(target_id, EqRelation::Neq))
                    .map(|n| path_store.get_path(n).map(|e| e.0).collect_vec())
            }
            EqRelation::Neq => {
                let mut path_store = PathStore::new();
                graph
                    .eq()
                    .traverse(source_id, &mut Default::default())
                    .mem_path(&mut path_store)
                    .find(|&n| n == target_id)
                    .map(|n| path_store.get_path(n).collect_vec())
            }
        }
        .unwrap_or_else(|| {
            let a_id = self.active_graph.get_id(&prop.a).unwrap();
            let b_id = self.active_graph.get_id(&prop.b).unwrap();
            panic!(
                "Unable to explain active graph: \n\
                    {}\n\
                    {}\n\
                    {:?}\n\
                    ({:?} -{}-> {:?}),\n\
                    ({:?} -{}-> {:?})",
                self.active_graph.to_graphviz(),
                self.active_graph.to_graphviz_grouped(),
                prop,
                a_id,
                prop.relation,
                b_id,
                self.active_graph.get_group_id(a_id),
                prop.relation,
                self.active_graph.get_group_id(b_id)
            )
        })
    }

    /// Explain an equality inference as a path of edges.
    pub fn eq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<IdEdge> {
        let source_id = self.active_graph.get_id(&Node::Var(literal.variable())).unwrap();

        let mut path_store = PathStore::new();
        let cause = self
            .active_graph
            .incoming
            .filter(|_, e| model.entails(e.active))
            .eq()
            .traverse(source_id, &mut Default::default())
            .mem_path(&mut path_store)
            .skip(1) // Cannot cause own propagation
            .find(|id| {
                let n = self.active_graph.get_node(*id);
                let (lb, ub) = model.node_bounds(&n);
                literal.svar().is_plus() && literal.variable().leq(ub).entails(literal)
                    || literal.svar().is_minus() && literal.variable().geq(lb).entails(literal)
            })
            .expect("Unable to explain eq propagation");
        path_store.get_path(cause).collect()
    }

    /// Explain a neq inference as a path of edges.
    pub fn neq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<IdEdge> {
        let source_id = self.active_graph.get_id(&Node::Var(literal.variable())).unwrap();

        let mut path_store = PathStore::new();
        let cause = self
            .active_graph
            .incoming
            .filter(|_, e| model.entails(e.active))
            .eq_neq()
            .traverse(EqNode::new(source_id), &mut Default::default())
            .mem_path(&mut path_store)
            .skip(1)
            .find(|EqNode(id, r)| {
                let (prev_lb, prev_ub) = model.bounds(literal.variable());
                // If relationship between node and literal node is Neq
                *r == EqRelation::Neq && {
                    let n = self.active_graph.get_node(*id);
                    // If node is bound to a value
                    if let Some(bound) = model.node_bound(&n) {
                        prev_ub == bound || prev_lb == bound
                    } else {
                        false
                    }
                }
            })
            .expect("Unable to explain Neq propagation");

        path_store.get_path(cause).map(|e| e.0).collect()
    }

    pub fn explain_from_path(
        &self,
        model: &DomainsSnapshot<'_>,
        literal: Lit,
        cause: ModelUpdateCause,
        path: Vec<IdEdge>,
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
