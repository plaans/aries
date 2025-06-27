#![allow(unused)]

use std::fmt::Debug;
use std::hash::Hash;

use itertools::Itertools;

use crate::reasoners::eq_alt::{
    core::EqRelation,
    graph::{
        adj_list::{AdjEdge, AdjNode, AdjacencyList},
        dft::Dft,
    },
};

mod adj_list;
mod dft;

pub(super) trait Label: Eq + Copy + Debug + Hash {}

impl<T: Eq + Copy + Debug + Hash> Label for T {}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub struct Edge<N: AdjNode, L: Label> {
    source: N,
    target: N,
    label: L,
    relation: EqRelation,
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
}

#[derive(Clone)]
pub(super) struct DirEqGraph<N: AdjNode, L: Label> {
    fwd_adj_list: AdjacencyList<N, Edge<N, L>>,
    rev_adj_list: AdjacencyList<N, Edge<N, L>>,
}

/// Directed pair of nodes with a == or != relation
#[derive(PartialEq, Eq, Hash, Debug)]
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
                 }| match relation {
                    EqRelation::Eq => !self.eq_path_exists(source, target),
                    EqRelation::Neq => !self.neq_path_exists(source, target),
                },
            )
    }

    fn paths_requiring_neq(&self, edge: Edge<N, L>) -> impl Iterator<Item = NodePair<N>> + use<'_, N, L> {
        let predecessors = Dft::new(&self.rev_adj_list, edge.source, (), |_, e| match e.relation {
            EqRelation::Eq => Some(()),
            EqRelation::Neq => None,
        });
        let successors = Dft::new(&self.fwd_adj_list, edge.target, (), |_, e| match e.relation {
            EqRelation::Eq => Some(()),
            EqRelation::Neq => None,
        });

        predecessors
            .cartesian_product(successors)
            .filter(|((source, _), (target, _))| !self.neq_path_exists(*source, *target))
            .map(|(p, s)| NodePair::new(p.0, s.0, EqRelation::Neq))
    }

    /// Util for Dft only on eq edges
    fn eq_dft(
        adj_list: &AdjacencyList<N, Edge<N, L>>,
        node: N,
    ) -> impl Iterator<Item = N> + Clone + Debug + use<'_, N, L> {
        Dft::new(adj_list, node, (), |_, e| match e.relation {
            EqRelation::Eq => Some(()),
            EqRelation::Neq => None,
        })
        .map(|(e, _)| e)
    }

    /// Util for Dft while 0 or 1 neqs
    fn eq_or_neq_dft(
        adj_list: &AdjacencyList<N, Edge<N, L>>,
        node: N,
    ) -> impl Iterator<Item = (N, EqRelation)> + Clone + use<'_, N, L> {
        Dft::new(adj_list, node, EqRelation::Eq, |r, e| r + e.relation)
    }
}

#[cfg(test)]
mod test {
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

    // #[test]
    // fn test_paths_requiring() {
    //     let mut g = DirEqGraph::new();
    //     // 0 -> 1
    //     g.add_edge(Edge::new(Node(0), Node(1), ()));
    //     // 2 --> 3
    //     g.add_edge(Edge::new(Node(2), Node(3), ()));

    //     // paths requiring
    //     assert_eq!(
    //         g.get_paths_requiring(Edge::new(Node(1), Node(2), ()))
    //             .collect::<HashSet<_>>(),
    //         [
    //             (Node(0), Node(2)).into(),
    //             (Node(0), Node(3)).into(),
    //             (Node(1), Node(2)).into(),
    //             (Node(1), Node(3)).into()
    //         ]
    //         .into()
    //     )
    // }
}
