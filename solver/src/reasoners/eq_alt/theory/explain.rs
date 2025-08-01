use crate::{
    core::{
        state::{DomainsSnapshot, Explanation},
        Lit,
    },
    reasoners::eq_alt::{
        graph::{
            folds::{EqFold, EqOrNeqFold},
            subsets::ActiveGraphSnapshot,
            traversal::GraphTraversal,
            GraphDir, IdEdge, TaggedNode,
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
        let graph = ActiveGraphSnapshot::new(model, self.active_graph.get_traversal_graph(GraphDir::Forward));
        match prop.relation {
            EqRelation::Eq => {
                let mut traversal = GraphTraversal::new(graph, EqOrNeqFold(), source_id, true);
                traversal
                    .find(|&TaggedNode(n, r)| n == target_id && r == EqRelation::Neq)
                    .map(|n| traversal.get_path(n))
            }
            EqRelation::Neq => {
                let mut traversal = GraphTraversal::new(graph, EqFold(), source_id, true);
                traversal
                    .find(|&TaggedNode(n, ..)| n == target_id)
                    .map(|n| traversal.get_path(n))
            }
        }
        .unwrap_or_else(|| {
            panic!(
                "Unable to explain active graph\n{}\n{:?}",
                self.active_graph.to_graphviz(),
                prop
            )
        })
    }

    /// Explain an equality inference as a path of edges.
    pub fn eq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<IdEdge> {
        let source_id = self.active_graph.get_id(&Node::Var(literal.variable())).unwrap();
        let mut traversal = GraphTraversal::new(
            ActiveGraphSnapshot::new(model, self.active_graph.get_traversal_graph(GraphDir::Reverse)),
            EqFold(),
            source_id,
            true,
        );
        // Node can't be it's own update cause
        traversal.next();
        let cause = traversal
            .find(|TaggedNode(id, _)| {
                let n = self.active_graph.get_node(*id);
                let (lb, ub) = model.node_bounds(&n);
                literal.svar().is_plus() && literal.variable().leq(ub).entails(literal)
                    || literal.svar().is_minus() && literal.variable().geq(lb).entails(literal)
            })
            // .flamap(|TaggedNode(n, r)| dft.get_path(TaggedNode(n, r)))
            .expect("Unable to explain eq propagation.");
        traversal.get_path(cause)
    }

    /// Explain a neq inference as a path of edges.
    pub fn neq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<IdEdge> {
        let source_id = self.active_graph.get_id(&Node::Var(literal.variable())).unwrap();
        let mut traversal = GraphTraversal::new(
            ActiveGraphSnapshot::new(model, self.active_graph.get_traversal_graph(GraphDir::Reverse)),
            EqOrNeqFold(),
            source_id,
            true,
        );
        // Node can't be it's own update cause
        traversal.next();
        let cause = traversal
            .find(|TaggedNode(id, r)| {
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
            .expect("Unable to explain neq propagation.");

        traversal.get_path(cause)
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
