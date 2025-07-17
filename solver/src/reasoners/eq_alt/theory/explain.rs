use crate::{
    core::{
        state::{DomainsSnapshot, Explanation},
        Lit,
    },
    reasoners::eq_alt::{
        graph::Edge, node::Node, propagators::PropagatorId, relation::EqRelation, theory::cause::ModelUpdateCause,
    },
};

use super::AltEqTheory;

impl AltEqTheory {
    /// Util closure used to filter edges that were not active at the time
    // TODO: Maybe also check is valid
    fn graph_filter_closure<'a>(model: &'a DomainsSnapshot<'a>) -> impl Fn(&Edge<Node>) -> bool + use<'a> {
        |e: &Edge<Node>| model.entails(e.active)
    }

    /// Explain a neq cycle inference as a path of edges.
    pub fn neq_cycle_explanation_path(&self, propagator_id: PropagatorId, model: &DomainsSnapshot) -> Vec<Edge<Node>> {
        let prop = self.constraint_store.get_propagator(propagator_id);
        let edge: Edge<Node> = prop.clone().into();
        match prop.relation {
            EqRelation::Eq => {
                self.active_graph
                    .get_neq_path(edge.target, edge.source, Self::graph_filter_closure(model))
            }
            EqRelation::Neq => {
                self.active_graph
                    .get_eq_path(edge.target, edge.source, Self::graph_filter_closure(model))
            }
        }
        .unwrap_or_else(|| {
            panic!(
                "Unable to explain active graph\n{}\n{:?}",
                self.active_graph.to_graphviz(),
                edge
            )
        })
    }

    /// Explain an equality inference as a path of edges.
    pub fn eq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<Edge<Node>> {
        let mut dft = self
            .active_graph
            .rev_eq_dft_path(Node::Var(literal.variable()), Self::graph_filter_closure(model));
        dft.next();
        dft.find(|(n, _)| {
            let (lb, ub) = model.get_node_bounds(n);
            literal.svar().is_plus() && literal.variable().leq(ub).entails(literal)
                || literal.svar().is_minus() && literal.variable().geq(lb).entails(literal)
        })
        .map(|(n, r)| dft.get_path(n, r))
        .expect("Unable to explain eq propagation.")
    }

    /// Explain a neq inference as a path of edges.
    pub fn neq_explanation_path(&self, literal: Lit, model: &DomainsSnapshot<'_>) -> Vec<Edge<Node>> {
        let mut dft = self
            .active_graph
            .rev_eq_or_neq_dft_path(Node::Var(literal.variable()), Self::graph_filter_closure(model));
        dft.find(|(n, r)| {
            let (prev_lb, prev_ub) = model.bounds(literal.variable());
            // If relationship between node and literal node is Neq
            *r == EqRelation::Neq && {
                // If node is bound to a value
                if let Some(bound) = model.get_node_bound(n) {
                    prev_ub == bound || prev_lb == bound
                } else {
                    false
                }
            }
        })
        .map(|(n, r)| dft.get_path(n, r))
        .expect("Unable to explain neq propagation.")
    }

    pub fn explain_from_path(
        &self,
        model: &DomainsSnapshot<'_>,
        literal: Lit,
        cause: ModelUpdateCause,
        path: Vec<Edge<Node>>,
        out_explanation: &mut Explanation,
    ) {
        use ModelUpdateCause::*;
        out_explanation.extend(path.iter().map(|e| e.active));

        // Eq will also require the ub/lb of the literal which is at the "origin" of the propagation
        // (If the node is a varref)
        if cause == DomEq || cause == DomNeq {
            let origin = path
                .first()
                .expect("Node cannot be at the origin of it's own inference.")
                .target;
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
