use std::fmt::Debug;
use std::hash::Hash;

use itertools::Itertools;

use crate::reasoners::eq_alt::{
    core::EqRelation,
    graph::{
        adj_list::{AdjEdge, AdjNode, AdjacencyList},
        bft::Bft,
    },
};

mod adj_list;
mod bft;

pub(super) trait Label: Eq + Copy + Debug + Hash {}

impl<T: Eq + Copy + Debug + Hash> Label for T {}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub struct Edge<N: AdjNode, L: Label> {
    pub source: N,
    pub target: N,
    pub label: L,
    pub relation: EqRelation,
}

impl<N: AdjNode, L: Label> Edge<N, L> {
    pub fn new(source: N, target: N, label: L, relation: EqRelation) -> Self {
        Self {
            source,
            target,
            label,
            relation,
        }
    }

    pub fn reverse(&self) -> Self {
        Edge {
            source: self.target,
            target: self.source,
            label: self.label,
            relation: self.relation,
        }
    }
}

impl<N: AdjNode, L: Label> AdjEdge<N> for Edge<N, L> {
    fn target(&self) -> N {
        self.target
    }

    fn source(&self) -> N {
        self.source
    }
}

#[derive(Clone, Debug)]
pub(super) struct DirEqGraph<N: AdjNode, L: Label> {
    fwd_adj_list: AdjacencyList<N, Edge<N, L>>,
    rev_adj_list: AdjacencyList<N, Edge<N, L>>,
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

impl<N: AdjNode, L: Label> DirEqGraph<N, L> {
    pub fn new() -> Self {
        Self {
            fwd_adj_list: AdjacencyList::new(),
            rev_adj_list: AdjacencyList::new(),
        }
    }

    pub fn add_edge(&mut self, edge: Edge<N, L>) {
        self.fwd_adj_list.insert_edge(edge.source, edge);
        self.rev_adj_list.insert_edge(edge.target, edge.reverse());
    }

    pub fn add_node(&mut self, node: N) {
        self.fwd_adj_list.insert_node(node);
        self.rev_adj_list.insert_node(node);
    }

    pub fn remove_edge(&mut self, edge: Edge<N, L>) {
        self.fwd_adj_list.remove_edge(edge.source, edge);
        self.rev_adj_list.remove_edge(edge.target, edge.reverse());
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
        filter: impl Fn(&Edge<N, L>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N, L>, (), impl Fn(&(), &Edge<N, L>) -> Option<()>> {
        Self::eq_path_dft(&self.rev_adj_list, source, filter)
    }

    /// Return an iterator over nodes which can be reached with Neq in reverse adjacency list
    #[allow(clippy::type_complexity)] // Impossible to simplify type due to unstable type alias features
    pub fn rev_eq_or_neq_dft_path<'a>(
        &'a self,
        source: N,
        filter: impl Fn(&Edge<N, L>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N, L>, EqRelation, impl Fn(&EqRelation, &Edge<N, L>) -> Option<EqRelation>> {
        Self::eq_or_neq_path_dft(&self.rev_adj_list, source, filter)
    }

    /// Get a path with EqRelation::Eq from source to target
    pub fn get_eq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N, L>) -> bool) -> Option<Vec<Edge<N, L>>> {
        let mut dft = Self::eq_path_dft(&self.fwd_adj_list, source, filter);
        dft.find(|(n, _)| *n == target).map(|(n, _)| dft.get_path(n))
    }

    /// Get a path with EqRelation::Neq from source to target
    pub fn get_neq_path(&self, source: N, target: N, filter: impl Fn(&Edge<N, L>) -> bool) -> Option<Vec<Edge<N, L>>> {
        let mut dft = Self::eq_or_neq_path_dft(&self.fwd_adj_list, source, filter);
        dft.find(|(n, r)| *n == target && *r == EqRelation::Neq)
            .map(|(n, _)| dft.get_path(n))
    }

    #[allow(unused)]
    pub fn get_eq_or_neq_path(
        &self,
        source: N,
        target: N,
        filter: impl Fn(&Edge<N, L>) -> bool,
    ) -> Option<Vec<Edge<N, L>>> {
        let mut dft = Self::eq_or_neq_path_dft(&self.fwd_adj_list, source, filter);
        dft.find(|(n, _)| *n == target).map(|(n, _)| dft.get_path(n))
    }

    /// Get all paths which would require the given edge to exist.
    /// Edge should not be already present in graph
    ///
    /// For an edge x -==-> y, returns a vec of all pairs (w, z) such that w -=-> z or w -!=-> z in G union x -=-> y, but not in G.
    ///
    /// For an edge x -!=-> y, returns a vec of all pairs (w, z) such that w -!=> z in G union x -!=-> y, but not in G.
    pub fn paths_requiring(&self, edge: Edge<N, L>) -> Box<dyn Iterator<Item = NodePair<N>> + '_> {
        // Brute force algo: Form pairs from all antecedants of x and successors of y
        // Then check if a path exists in graph
        match edge.relation {
            EqRelation::Eq => Box::new(self.paths_requiring_eq(edge)),
            EqRelation::Neq => Box::new(self.paths_requiring_neq(edge)),
        }
    }

    pub fn iter_all_fwd(&self) -> impl Iterator<Item = Edge<N, L>> + use<'_, N, L> {
        self.fwd_adj_list.iter_all_edges()
    }

    fn paths_requiring_eq(&self, edge: Edge<N, L>) -> impl Iterator<Item = NodePair<N>> + use<'_, N, L> {
        let predecessors = Self::eq_or_neq_dft(&self.rev_adj_list, edge.source);
        let successors = Self::eq_or_neq_dft(&self.fwd_adj_list, edge.target);

        predecessors
            .cartesian_product(successors)
            .filter_map(|(p, s)| Some(NodePair::new(p.0, s.0, (p.1 + s.1)?)))
            .filter(
                |&NodePair {
                     source,
                     target,
                     relation,
                 }| {
                    match relation {
                        EqRelation::Eq => !self.eq_path_exists(source, target),
                        EqRelation::Neq => !self.neq_path_exists(source, target),
                    }
                },
            )
    }

    fn paths_requiring_neq(&self, edge: Edge<N, L>) -> impl Iterator<Item = NodePair<N>> + use<'_, N, L> {
        let predecessors = Self::eq_dft(&self.rev_adj_list, edge.source);
        let successors = Self::eq_dft(&self.fwd_adj_list, edge.target);

        predecessors
            .cartesian_product(successors)
            .filter(|(source, target)| *source != *target && !self.neq_path_exists(*source, *target))
            .map(|(p, s)| NodePair::new(p, s, EqRelation::Neq))
    }

    /// Util for Dft only on eq edges
    fn eq_dft(adj_list: &AdjacencyList<N, Edge<N, L>>, node: N) -> impl Iterator<Item = N> + Clone + use<'_, N, L> {
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
        adj_list: &AdjacencyList<N, Edge<N, L>>,
        node: N,
    ) -> impl Iterator<Item = (N, EqRelation)> + Clone + use<'_, N, L> {
        Bft::new(adj_list, node, EqRelation::Eq, |r, e| *r + e.relation, false)
    }

    #[allow(clippy::type_complexity)] // Impossible to simplify type due to unstable type alias features
    fn eq_path_dft<'a>(
        adj_list: &'a AdjacencyList<N, Edge<N, L>>,
        node: N,
        filter: impl Fn(&Edge<N, L>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N, L>, (), impl Fn(&(), &Edge<N, L>) -> Option<()>> {
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
        adj_list: &'a AdjacencyList<N, Edge<N, L>>,
        node: N,
        filter: impl Fn(&Edge<N, L>) -> bool + 'a,
    ) -> Bft<'a, N, Edge<N, L>, EqRelation, impl Fn(&EqRelation, &Edge<N, L>) -> Option<EqRelation>> {
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

    pub(crate) fn creates_neq_cycle(&self, edge: Edge<N, L>) -> bool {
        match edge.relation {
            EqRelation::Eq => self.neq_path_exists(edge.target, edge.source),
            EqRelation::Neq => self.eq_path_exists(edge.target, edge.source),
        }
    }

    #[allow(unused)]
    pub(crate) fn print_allocated(&self) {
        println!("Fwd allocated: {}", self.fwd_adj_list.allocated());
        println!("Rev allocated: {}", self.rev_adj_list.allocated());
    }
}

#[cfg(test)]
mod tests {
    use hashbrown::HashSet;

    use super::*;

    #[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
    struct Node(u32);

    #[test]
    fn test_path_exists() {
        let mut g = DirEqGraph::new();
        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), (), EqRelation::Eq));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), (), EqRelation::Neq));
        // 2 -=-> 3
        g.add_edge(Edge::new(Node(2), Node(3), (), EqRelation::Eq));
        // 2 -!=-> 4
        g.add_edge(Edge::new(Node(2), Node(4), (), EqRelation::Neq));

        // 0 -=-> 3
        assert!(g.eq_path_exists(Node(0), Node(3)));

        // 0 -!=-> 4
        assert!(g.neq_path_exists(Node(0), Node(4)));

        // !1 -!=-> 4 && !1 -==-> 4
        assert!(!g.eq_path_exists(Node(1), Node(4)) && !g.neq_path_exists(Node(1), Node(4)));

        // 3 -=-> 0
        g.add_edge(Edge::new(Node(3), Node(0), (), EqRelation::Eq));
        assert!(g.eq_path_exists(Node(2), Node(0)));
    }

    #[test]
    fn test_paths_requiring() {
        let mut g = DirEqGraph::new();

        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), (), EqRelation::Eq));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), (), EqRelation::Neq));
        // 3 -=-> 4
        g.add_edge(Edge::new(Node(3), Node(4), (), EqRelation::Eq));
        // 3 -!=-> 5
        g.add_edge(Edge::new(Node(3), Node(5), (), EqRelation::Neq));
        // 0 -=-> 4
        g.add_edge(Edge::new(Node(0), Node(4), (), EqRelation::Eq));

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
            g.paths_requiring(Edge::new(Node(2), Node(3), (), EqRelation::Eq))
                .collect::<HashSet<_>>(),
            res
        );

        g.add_edge(Edge::new(Node(2), Node(3), (), EqRelation::Eq));
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), (), EqRelation::Eq))
                .collect::<HashSet<_>>(),
            [].into()
        );

        g.remove_edge(Edge::new(Node(2), Node(3), (), EqRelation::Eq));
        assert_eq!(
            g.paths_requiring(Edge::new(Node(2), Node(3), (), EqRelation::Eq))
                .collect::<HashSet<_>>(),
            res
        );
    }

    #[test]
    fn test_path() {
        let mut g = DirEqGraph::new();

        // 0 -=-> 2
        g.add_edge(Edge::new(Node(0), Node(2), (), EqRelation::Eq));
        // 1 -!=-> 2
        g.add_edge(Edge::new(Node(1), Node(2), (), EqRelation::Neq));
        // 3 -=-> 4
        g.add_edge(Edge::new(Node(3), Node(4), (), EqRelation::Eq));
        // 3 -!=-> 5
        g.add_edge(Edge::new(Node(3), Node(5), (), EqRelation::Neq));
        // 0 -=-> 4
        g.add_edge(Edge::new(Node(0), Node(4), (), EqRelation::Eq));

        let path = g.get_neq_path(Node(0), Node(5), |_| true);
        assert_eq!(path, None);

        g.add_edge(Edge::new(Node(2), Node(3), (), EqRelation::Eq));

        let path = g.get_neq_path(Node(0), Node(5), |_| true);
        assert_eq!(
            path,
            vec![
                Edge::new(Node(3), Node(5), (), EqRelation::Neq),
                Edge::new(Node(2), Node(3), (), EqRelation::Eq),
                Edge::new(Node(0), Node(2), (), EqRelation::Eq)
            ]
            .into()
        );
    }

    #[test]
    fn test_single_node() {
        let mut g: DirEqGraph<Node, ()> = DirEqGraph::new();
        g.add_node(Node(1));
        assert!(g.eq_path_exists(Node(1), Node(1)));
        assert!(!g.neq_path_exists(Node(1), Node(1)));
    }
}
