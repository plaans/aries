use std::fmt::{Debug, Display};
use std::hash::Hash;

use hashbrown::HashSet;
use itertools::Itertools;

use crate::core::Lit;
use crate::reasoners::eq_alt::graph::{
    adj_list::{AdjEdge, AdjNode, AdjacencyList},
    bft::Bft,
};

use super::relation::EqRelation;

mod adj_list;
mod bft;

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub struct Edge<N: AdjNode> {
    pub source: N,
    pub target: N,
    pub active: Lit,
    pub relation: EqRelation,
}

impl<N: AdjNode> Edge<N> {
    pub fn new(source: N, target: N, active: Lit, relation: EqRelation) -> Self {
        Self {
            source,
            target,
            active,
            relation,
        }
    }

    pub fn reverse(&self) -> Self {
        Edge {
            source: self.target,
            target: self.source,
            active: self.active,
            relation: self.relation,
        }
    }
}

impl<N: AdjNode> AdjEdge<N> for Edge<N> {
    fn target(&self) -> N {
        self.target
    }

    fn source(&self) -> N {
        self.source
    }
}

#[derive(Clone, Debug)]
pub(super) struct DirEqGraph<N: AdjNode> {
    fwd_adj_list: AdjacencyList<N, Edge<N>>,
    rev_adj_list: AdjacencyList<N, Edge<N>>,
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
            fwd_adj_list: AdjacencyList::new(),
            rev_adj_list: AdjacencyList::new(),
        }
    }

    pub fn get_fwd_out_edges(&self, node: N) -> Option<&HashSet<Edge<N>>> {
        self.fwd_adj_list.get_edges(node)
    }

    pub fn add_edge(&mut self, edge: Edge<N>) {
        self.fwd_adj_list.insert_edge(edge.source, edge);
        self.rev_adj_list.insert_edge(edge.target, edge.reverse());
    }

    pub fn add_node(&mut self, node: N) {
        self.fwd_adj_list.insert_node(node);
        self.rev_adj_list.insert_node(node);
    }

    pub fn contains_edge(&self, edge: Edge<N>) -> bool {
        self.fwd_adj_list.contains_edge(edge)
    }

    pub fn remove_edge(&mut self, edge: Edge<N>) -> bool {
        self.fwd_adj_list.remove_edge(edge.source, edge) && self.rev_adj_list.remove_edge(edge.target, edge.reverse())
    }

    // Returns true if source -=-> target
    pub fn eq_path_exists(&self, source: N, target: N) -> bool {
        Self::eq_dft(&self.fwd_adj_list, source).any(|e| e == target)
    }

    // Returns true if source -!=-> target
    pub fn neq_path_exists(&self, source: N, target: N) -> bool {
        Self::eq_or_neq_dft(&self.fwd_adj_list, source).any(|(e, r)| e == target && r == EqRelation::Neq)
    }

    /// Return a Dft struct over nodes which can be reached with Eq in reverse adjacency list
    #[allow(clippy::type_complexity)] // Impossible to simplify type due to unstable type alias features
    pub fn rev_eq_dft_path<'a>(
        &'a self,
        source: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N>, (), impl Fn(&(), &Edge<N>) -> Option<()>> {
        Self::eq_path_dft(&self.rev_adj_list, source, filter)
    }

    /// Return an iterator over nodes which can be reached with Neq in reverse adjacency list
    #[allow(clippy::type_complexity)] // Impossible to simplify type due to unstable type alias features
    pub fn rev_eq_or_neq_dft_path<'a>(
        &'a self,
        source: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N>, EqRelation, impl Fn(&EqRelation, &Edge<N>) -> Option<EqRelation>> {
        Self::eq_or_neq_path_dft(&self.rev_adj_list, source, filter)
    }

    /// Get a path with EqRelation::Eq from source to target
    pub fn get_eq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N>) -> bool) -> Option<Vec<Edge<N>>> {
        let mut dft = Self::eq_path_dft(&self.fwd_adj_list, source, filter);
        dft.find(|(n, _)| *n == target).map(|(n, _)| dft.get_path(n, ()))
    }

    /// Get a path with EqRelation::Neq from source to target
    pub fn get_neq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N>) -> bool) -> Option<Vec<Edge<N>>> {
        let mut dft = Self::eq_or_neq_path_dft(&self.fwd_adj_list, source, filter);
        dft.find(|(n, r)| *n == target && *r == EqRelation::Neq)
            .map(|(n, _)| dft.get_path(n, EqRelation::Neq))
    }

    #[allow(unused)]
    pub fn get_eq_or_neq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N>) -> bool) -> Option<Vec<Edge<N>>> {
        let mut dft = Self::eq_or_neq_path_dft(&self.fwd_adj_list, source, filter);
        dft.find(|(n, _)| *n == target).map(|(n, r)| dft.get_path(n, r))
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

    pub fn iter_all_fwd(&self) -> impl Iterator<Item = Edge<N>> + use<'_, N> {
        self.fwd_adj_list.iter_all_edges()
    }

    fn paths_requiring_eq(&self, edge: Edge<N>) -> impl Iterator<Item = NodePair<N>> + use<'_, N> {
        let predecessors = Self::eq_or_neq_dft(&self.rev_adj_list, edge.source);
        let successors = Self::eq_or_neq_dft(&self.fwd_adj_list, edge.target);

        predecessors
            .cartesian_product(successors)
            .filter_map(|(p, s)| Some(NodePair::new(p.0, s.0, (p.1 + s.1)?)))
            .filter(|np| match np.relation {
                EqRelation::Eq => !self.eq_path_exists(np.source, np.target),
                EqRelation::Neq => !self.neq_path_exists(np.source, np.target),
            })
    }

    fn paths_requiring_neq(&self, edge: Edge<N>) -> impl Iterator<Item = NodePair<N>> + use<'_, N> {
        let predecessors = Self::eq_dft(&self.rev_adj_list, edge.source);
        let successors = Self::eq_dft(&self.fwd_adj_list, edge.target);

        predecessors
            .cartesian_product(successors)
            .filter(|(source, target)| *source != *target && !self.neq_path_exists(*source, *target))
            .map(|(p, s)| NodePair::new(p, s, EqRelation::Neq))
    }

    /// Util for Dft only on eq edges
    fn eq_dft(adj_list: &AdjacencyList<N, Edge<N>>, node: N) -> impl Iterator<Item = N> + Clone + use<'_, N> {
        Bft::new(
            adj_list,
            node,
            (),
            |_, e| match e.relation {
                EqRelation::Eq => Some(()),
                EqRelation::Neq => None,
            },
            false,
        )
        .map(|(e, _)| e)
    }

    /// Util for Dft while 0 or 1 neqs
    fn eq_or_neq_dft(
        adj_list: &AdjacencyList<N, Edge<N>>,
        node: N,
    ) -> impl Iterator<Item = (N, EqRelation)> + Clone + use<'_, N> {
        Bft::new(adj_list, node, EqRelation::Eq, move |r, e| *r + e.relation, false)
    }

    #[allow(clippy::type_complexity)] // Impossible to simplify type due to unstable type alias features
    fn eq_path_dft<'a>(
        adj_list: &'a AdjacencyList<N, Edge<N>>,
        node: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N>, (), impl Fn(&(), &Edge<N>) -> Option<()>> {
        Bft::new(
            adj_list,
            node,
            (),
            move |_, e| {
                if filter(e) {
                    match e.relation {
                        EqRelation::Eq => Some(()),
                        EqRelation::Neq => None,
                    }
                } else {
                    None
                }
            },
            true,
        )
    }

    /// Util for Dft while 0 or 1 neqs
    #[allow(clippy::type_complexity)] // Impossible to simplify type due to unstable type alias features
    fn eq_or_neq_path_dft<'a>(
        adj_list: &'a AdjacencyList<N, Edge<N>>,
        node: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N>, EqRelation, impl Fn(&EqRelation, &Edge<N>) -> Option<EqRelation>> {
        Bft::new(
            adj_list,
            node,
            EqRelation::Eq,
            move |r, e| {
                if filter(e) {
                    *r + e.relation
                } else {
                    None
                }
            },
            true,
        )
    }

    #[allow(unused)]
    pub(crate) fn print_allocated(&self) {
        println!("Fwd allocated: {}", self.fwd_adj_list.allocated());
        println!("Rev allocated: {}", self.rev_adj_list.allocated());
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
                e.source(),
                e.target(),
                e.relation,
                e.active
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
    struct Node(u32);

    impl Display for Node {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    #[test]
    fn test_path_exists() {
        let mut g = DirEqGraph::new();
        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), Lit::TRUE, EqRelation::Neq));
        // 2 -=-> 3
        g.add_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq));
        // 2 -!=-> 4
        g.add_edge(Edge::new(Node(2), Node(4), Lit::TRUE, EqRelation::Neq));

        // 0 -=-> 3
        assert!(g.eq_path_exists(Node(0), Node(3)));

        // 0 -!=-> 4
        assert!(g.neq_path_exists(Node(0), Node(4)));

        // !1 -!=-> 4 && !1 -==-> 4
        assert!(!g.eq_path_exists(Node(1), Node(4)) && !g.neq_path_exists(Node(1), Node(4)));

        // 3 -=-> 0
        g.add_edge(Edge::new(Node(3), Node(0), Lit::TRUE, EqRelation::Eq));
        assert!(g.eq_path_exists(Node(2), Node(0)));
    }

    #[test]
    fn test_paths_requiring() {
        let mut g = DirEqGraph::new();

        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), Lit::TRUE, EqRelation::Neq));
        // 3 -=-> 4
        g.add_edge(Edge::new(Node(3), Node(4), Lit::TRUE, EqRelation::Eq));
        // 3 -!=-> 5
        g.add_edge(Edge::new(Node(3), Node(5), Lit::TRUE, EqRelation::Neq));
        // 0 -=-> 4
        g.add_edge(Edge::new(Node(0), Node(4), Lit::TRUE, EqRelation::Eq));

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
            g.paths_requiring(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq))
                .collect::<HashSet<_>>(),
            res
        );

        g.add_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq));
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq))
                .collect::<HashSet<_>>(),
            [].into()
        );

        g.remove_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq));
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq))
                .collect::<HashSet<_>>(),
            res
        );
    }

    #[test]
    fn test_path() {
        let mut g = DirEqGraph::new();

        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), Lit::TRUE, EqRelation::Neq));
        // 3 -=-> 4
        g.add_edge(Edge::new(Node(3), Node(4), Lit::TRUE, EqRelation::Eq));
        // 3 -!=-> 5
        g.add_edge(Edge::new(Node(3), Node(5), Lit::TRUE, EqRelation::Neq));
        // 0 -=-> 4
        g.add_edge(Edge::new(Node(0), Node(4), Lit::TRUE, EqRelation::Eq));

        let path = g.get_neq_path(Node(0), Node(5), |_| true);
        assert_eq!(path, None);

        g.add_edge(Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq));

        let path = g.get_neq_path(Node(0), Node(5), |_| true);
        assert_eq!(
            path,
            vec![
                Edge::new(Node(3), Node(5), Lit::TRUE, EqRelation::Neq),
                Edge::new(Node(2), Node(3), Lit::TRUE, EqRelation::Eq),
                Edge::new(Node(0), Node(2), Lit::TRUE, EqRelation::Eq)
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
