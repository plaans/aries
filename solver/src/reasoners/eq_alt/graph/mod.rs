use std::fmt::{Debug, Display};
use std::hash::Hash;

use itertools::Itertools;
pub use traversal::TaggedNode;

use crate::core::Lit;
use crate::reasoners::eq_alt::graph::{
    adj_list::{AdjNode, EqAdjList},
    traversal::GraphTraversal,
};

use super::node::Node;
use super::propagators::{Propagator, PropagatorId};
use super::relation::EqRelation;

mod adj_list;
mod traversal;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub struct Edge<N: AdjNode> {
    pub source: N,
    pub target: N,
    pub active: Lit,
    pub relation: EqRelation,
    pub prop_id: PropagatorId,
}

impl Edge<Node> {
    pub fn from_prop(prop_id: PropagatorId, prop: Propagator) -> Self {
        Self {
            prop_id,
            source: prop.a,
            target: prop.b,
            active: prop.enabler.active,
            relation: prop.relation,
        }
    }
}

impl<N: AdjNode> Edge<N> {
    pub fn new(source: N, target: N, active: Lit, relation: EqRelation, prop_id: PropagatorId) -> Self {
        Self {
            source,
            target,
            active,
            relation,
            prop_id,
        }
    }

    /// Should only be used for reverse adjacency graph. Propagator id is not reversed.
    pub fn reverse(&self) -> Self {
        Edge {
            source: self.target,
            target: self.source,
            active: self.active,
            relation: self.relation,
            prop_id: self.prop_id,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct DirEqGraph<N: AdjNode> {
    fwd_adj_list: EqAdjList<N>,
    rev_adj_list: EqAdjList<N>,
}

/// Directed pair of nodes with a == or != relation
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct NodePair<N> {
    pub source: N,
    pub target: N,
    pub relation: EqRelation,
}

impl<N> NodePair<N> {
    pub fn new(source: N, target: N, relation: EqRelation) -> Self {
        Self {
            source,
            target,
            relation,
        }
    }
}

impl<N> From<(N, N, EqRelation)> for NodePair<N> {
    fn from(val: (N, N, EqRelation)) -> Self {
        NodePair {
            source: val.0,
            target: val.1,
            relation: val.2,
        }
    }
}

impl<N: AdjNode> DirEqGraph<N> {
    pub fn new() -> Self {
        Self {
            fwd_adj_list: EqAdjList::new(),
            rev_adj_list: EqAdjList::new(),
        }
    }

    pub fn add_edge(&mut self, edge: Edge<N>) {
        self.fwd_adj_list.insert_edge(edge.source, edge);
        self.rev_adj_list.insert_edge(edge.target, edge.reverse());
    }

    pub fn add_node(&mut self, node: N) {
        self.fwd_adj_list.insert_node(node);
        self.rev_adj_list.insert_node(node);
    }

    pub fn remove_edge(&mut self, edge: Edge<N>) {
        self.fwd_adj_list.remove_edge(edge.source, edge);
        self.rev_adj_list.remove_edge(edge.target, edge.reverse())
    }

    // Returns true if source -=-> target
    pub fn eq_path_exists(&self, source: N, target: N) -> bool {
        self.fwd_adj_list
            .eq_traversal(source, |_| true)
            .any(|TaggedNode(e, _)| e == target)
    }

    // Returns true if source -!=-> target
    pub fn neq_path_exists(&self, source: N, target: N) -> bool {
        self.fwd_adj_list
            .eq_or_neq_traversal(source, |_, _| true)
            .any(|TaggedNode(e, r)| e == target && r == EqRelation::Neq)
    }

    /// Return a Dft struct over nodes which can be reached with Eq in reverse adjacency list
    pub fn rev_eq_dft_path<'a>(
        &'a self,
        source: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> GraphTraversal<'a, N, bool, impl Fn(&bool, &Edge<N>) -> Option<bool>> {
        self.rev_adj_list.eq_path_traversal(source, filter)
    }

    /// Return an iterator over nodes which can be reached with Neq in reverse adjacency list
    pub fn rev_eq_or_neq_dft_path<'a>(
        &'a self,
        source: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> GraphTraversal<'a, N, EqRelation, impl Fn(&EqRelation, &Edge<N>) -> Option<EqRelation>> {
        self.rev_adj_list.eq_or_neq_path_traversal(source, filter)
    }

    /// Get a path with EqRelation::Eq from source to target
    pub fn get_eq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N>) -> bool) -> Option<Vec<Edge<N>>> {
        let mut dft = self.fwd_adj_list.eq_path_traversal(source, filter);
        dft.find(|TaggedNode(n, _)| *n == target)
            .map(|TaggedNode(n, _)| dft.get_path(TaggedNode(n, false)))
    }

    /// Get a path with EqRelation::Neq from source to target
    pub fn get_neq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N>) -> bool) -> Option<Vec<Edge<N>>> {
        let mut dft = self.fwd_adj_list.eq_or_neq_path_traversal(source, filter);
        dft.find(|TaggedNode(n, r)| *n == target && *r == EqRelation::Neq)
            .map(|TaggedNode(n, _)| dft.get_path(TaggedNode(n, EqRelation::Neq)))
    }

    /// Get all paths which would require the given edge to exist.
    /// Edge should not be already present in graph
    ///
    /// For an edge x -==-> y, returns a vec of all pairs (w, z) such that w -=-> z or w -!=-> z in G union x -=-> y, but not in G.
    ///
    /// For an edge x -!=-> y, returns a vec of all pairs (w, z) such that w -!=> z in G union x -!=-> y, but not in G.
    pub fn paths_requiring(&self, edge: Edge<N>) -> Box<dyn Iterator<Item = NodePair<N>> + '_> {
        // Brute force algo: Form pairs from all antecedants of x and successors of y
        // Then check if a path exists in graph
        match edge.relation {
            EqRelation::Eq => Box::new(self.paths_requiring_eq(edge)),
            EqRelation::Neq => Box::new(self.paths_requiring_neq(edge)),
        }
    }

    fn paths_requiring_eq(&self, edge: Edge<N>) -> impl Iterator<Item = NodePair<N>> + use<'_, N> {
        let reachable_preds = self.rev_adj_list.eq_or_neq_reachable_from(edge.target);
        let reachable_succs = self.fwd_adj_list.eq_or_neq_reachable_from(edge.source);
        let predecessors = self.rev_adj_list.eq_or_neq_traversal(edge.source, move |e, r| {
            !reachable_preds.contains(TaggedNode(e.target, *r))
        });
        let successors = self
            .fwd_adj_list
            .eq_or_neq_traversal(edge.target, move |e, r| {
                !reachable_succs.contains(TaggedNode(e.target, *r))
            })
            .collect_vec();

        predecessors
            .cartesian_product(successors)
            .filter_map(|(p, s)| Some(NodePair::new(p.0, s.0, (p.1 + s.1)?)))
    }

    fn paths_requiring_neq(&self, edge: Edge<N>) -> impl Iterator<Item = NodePair<N>> + use<'_, N> {
        let reachable_preds = self.rev_adj_list.eq_reachable_from(edge.target);
        let reachable_succs = self.fwd_adj_list.eq_or_neq_reachable_from(edge.source);
        // let reachable_succs = self.fwd_adj_list.neq_reachable_from(edge.source);
        let predecessors = self
            .rev_adj_list
            .eq_traversal(edge.source, move |e| {
                !reachable_preds.contains(TaggedNode(e.target, false))
            })
            .map(|TaggedNode(e, _)| e);
        let successors = self
            .fwd_adj_list
            .eq_traversal(edge.target, move |e| {
                !reachable_succs.contains(TaggedNode(e.target, EqRelation::Neq))
            })
            .map(|TaggedNode(e, _)| e)
            .collect_vec();

        let res = predecessors
            .cartesian_product(successors)
            .map(|(p, s)| NodePair::new(p, s, EqRelation::Neq));

        // let reachable_preds = self.rev_adj_list.neq_reachable_from(edge.target);
        let reachable_preds = self.rev_adj_list.eq_or_neq_reachable_from(edge.target);
        let reachable_succs = self.fwd_adj_list.eq_reachable_from(edge.source);
        let predecessors = self
            .rev_adj_list
            .eq_traversal(edge.source, move |e| {
                !reachable_preds.contains(TaggedNode(e.target, EqRelation::Neq))
            })
            .map(|TaggedNode(e, _)| e);
        let successors = self
            .fwd_adj_list
            .eq_traversal(edge.target, move |e| {
                !reachable_succs.contains(TaggedNode(e.target, false))
            })
            .map(|TaggedNode(e, _)| e)
            .collect_vec();

        res.chain(
            predecessors
                .cartesian_product(successors)
                .map(|(p, s)| NodePair::new(p, s, EqRelation::Neq)),
        )
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = N> + use<'_, N> {
        self.fwd_adj_list.iter_nodes()
    }
}

impl<N: AdjNode + Display> DirEqGraph<N> {
    #[allow(unused)]
    pub(crate) fn to_graphviz(&self) -> String {
        let mut strings = vec!["digraph {".to_string()];
        for e in self.fwd_adj_list.iter_all_edges() {
            strings.push(format!(
                "  {} -> {} [label=\"{} ({:?})\"]",
                e.source, e.target, e.relation, e.active
            ));
        }
        strings.push("}".to_string());
        strings.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use hashbrown::HashSet;

    use super::*;

    #[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
    struct Node(usize);

    impl From<usize> for Node {
        fn from(value: usize) -> Self {
            Self(value)
        }
    }

    impl From<Node> for usize {
        fn from(value: Node) -> Self {
            value.0
        }
    }

    impl Display for Node {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    #[test]
    fn test_path_exists() {
        let mut g = DirEqGraph::new();
        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq, 0_u32.into()));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), Lit::TRUE, EqRelation::Neq, 1_u32.into()));
        // 2 -=-> 3
        g.add_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 2_u32.into()));
        // 2 -!=-> 4
        g.add_edge(Edge::new(Node(2), Node(4), Lit::TRUE, EqRelation::Neq, 3_u32.into()));

        // 0 -=-> 3
        assert!(g.eq_path_exists(Node(0), Node(3)));

        // 0 -!=-> 4
        assert!(g.neq_path_exists(Node(0), Node(4)));

        // !1 -!=-> 4 && !1 -==-> 4
        assert!(!g.eq_path_exists(Node(1), Node(4)) && !g.neq_path_exists(Node(1), Node(4)));

        // 3 -=-> 0
        g.add_edge(Edge::new(Node(3), Node(0), Lit::TRUE, EqRelation::Eq, 4_u32.into()));
        assert!(g.eq_path_exists(Node(2), Node(0)));
    }

    #[test]
    fn test_paths_requiring() {
        let mut g = DirEqGraph::new();

        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq, 0_u32.into()));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), Lit::TRUE, EqRelation::Neq, 1_u32.into()));
        // 3 -=-> 4
        g.add_edge(Edge::new(Node(3), Node(4), Lit::TRUE, EqRelation::Eq, 2_u32.into()));
        // 3 -!=-> 5
        g.add_edge(Edge::new(Node(3), Node(5), Lit::TRUE, EqRelation::Neq, 3_u32.into()));
        // 0 -=-> 4
        g.add_edge(Edge::new(Node(0), Node(4), Lit::TRUE, EqRelation::Eq, 3_u32.into()));

        let res = [
            (Node(0), Node(3), EqRelation::Eq).into(),
            (Node(0), Node(5), EqRelation::Neq).into(),
            (Node(1), Node(3), EqRelation::Neq).into(),
            (Node(1), Node(4), EqRelation::Neq).into(),
            (Node(2), Node(3), EqRelation::Eq).into(),
            (Node(2), Node(4), EqRelation::Eq).into(),
            (Node(2), Node(5), EqRelation::Neq).into(),
        ]
        .into();
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 0_u32.into()))
                .collect::<HashSet<_>>(),
            res
        );

        g.add_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 0_u32.into()));
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 0_u32.into()))
                .collect::<HashSet<_>>(),
            [].into()
        );

        g.remove_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 0_u32.into()));
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 0_u32.into()))
                .collect::<HashSet<_>>(),
            res
        );
    }

    #[test]
    fn test_path() {
        let mut g = DirEqGraph::new();

        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq, 0_u32.into()));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), Lit::TRUE, EqRelation::Neq, 1_u32.into()));
        // 3 -=-> 4
        g.add_edge(Edge::new(Node(3), Node(4), Lit::TRUE, EqRelation::Eq, 2_u32.into()));
        // 3 -!=-> 5
        g.add_edge(Edge::new(Node(3), Node(5), Lit::TRUE, EqRelation::Neq, 3_u32.into()));
        // 0 -=-> 4
        g.add_edge(Edge::new(Node(0), Node(4), Lit::TRUE, EqRelation::Eq, 4_u32.into()));

        let path = g.get_neq_path(Node(0), Node(5), |_| true);
        assert_eq!(path, None);

        g.add_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 5_u32.into()));

        let path = g.get_neq_path(Node(0), Node(5), |_| true);
        assert_eq!(
            path,
            vec![
                Edge::new(Node(3), Node(5), Lit::TRUE, EqRelation::Neq, 3_u32.into()),
                Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq, 5_u32.into()),
                Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq, 0_u32.into())
            ]
            .into()
        );
    }

    #[test]
    fn test_single_node() {
        let mut g: DirEqGraph<Node> = DirEqGraph::new();
        g.add_node(Node(1));
        assert!(g.eq_path_exists(Node(1), Node(1)));
        assert!(!g.neq_path_exists(Node(1), Node(1)));
    }
}
