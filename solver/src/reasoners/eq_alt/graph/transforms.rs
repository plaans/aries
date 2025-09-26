use crate::reasoners::eq_alt::relation::EqRelation;

use super::{
    traversal::{self},
    Edge, EqAdjList, NodeId, Path,
};

// Implementations of generic edge for concrete edge type
impl traversal::Edge<NodeId> for Edge {
    fn target(&self) -> NodeId {
        self.target
    }

    fn source(&self) -> NodeId {
        self.source
    }
}

// Implementation of generic graph for concrete graph
impl traversal::Graph<NodeId, Edge> for &EqAdjList {
    fn outgoing(&self, node: NodeId) -> impl Iterator<Item = Edge> {
        self.iter_edges(node).cloned()
    }
}

// Node with associated relation type
#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq)]
pub struct EqNode(pub NodeId, pub EqRelation);
impl EqNode {
    /// Returns EqNode with relation =
    pub fn new(source: NodeId) -> Self {
        Self(source, EqRelation::Eq)
    }

    pub fn negate(self) -> Self {
        Self(self.0, !self.1)
    }

    pub fn path_to(&self, other: &EqNode) -> Option<Path> {
        Some(Path::new(self.0, other.0, (self.1 + other.1)?))
    }
}

// Node trait implementation for Eq Node
// Relation gets first bit, N is shifted to the left by one

impl From<usize> for EqNode {
    fn from(value: usize) -> Self {
        let r = if value & 1 != 0 {
            EqRelation::Eq
        } else {
            EqRelation::Neq
        };
        Self((value >> 1).into(), r)
    }
}

impl From<EqNode> for usize {
    fn from(value: EqNode) -> Self {
        let shift = 1;
        let v = match value.1 {
            EqRelation::Eq => 1_usize,
            EqRelation::Neq => 0_usize,
        };
        v | usize::from(value.0) << shift
    }
}

/// EqEdge type that goes with EqNode for graph traversal.
///
/// Second field is the relation of the target node
/// (Hence the - in source)
#[derive(Debug, Clone)]
pub struct EqEdge(pub Edge, EqRelation);

impl traversal::Edge<EqNode> for EqEdge {
    fn target(&self) -> EqNode {
        EqNode(self.0.target, self.1)
    }

    fn source(&self) -> EqNode {
        EqNode(self.0.source, (self.1 - self.0.relation).unwrap())
    }
}

/// Filters the graph to only include edges with equality relation.
///
/// Commonly used when looking for nodes which are equal to the source
pub struct EqFilter<G: traversal::Graph<NodeId, Edge>>(G);

impl<G: traversal::Graph<NodeId, Edge>> traversal::Graph<NodeId, Edge> for EqFilter<G> {
    fn outgoing(&self, node: NodeId) -> impl Iterator<Item = Edge> {
        self.0.outgoing(node).filter(|e| e.relation == EqRelation::Eq)
    }
}

/// Extension trait used to add the eq method to implementations of Graph<NodeId, Edge>
pub trait EqExt<G: traversal::Graph<NodeId, Edge>> {
    /// Filters the graph to only include edges with equality relation.
    ///
    /// Commonly used when looking for nodes which are equal to the source
    fn eq(self) -> EqFilter<G>;
}
impl<G> EqExt<G> for G
where
    G: traversal::Graph<NodeId, Edge>,
{
    fn eq(self) -> EqFilter<G> {
        EqFilter(self)
    }
}

/// Transform the graph in order to traverse it following equality's transitivity laws.
///
/// Modifies the graph so that each node has two copies: One with Eq relation, and one with Neq relation.
///
/// Adapts edges so that a -=> b && b -!=-> c, a -!=-> c and so on.
pub struct EqNeqFilter<G: traversal::Graph<NodeId, Edge>>(G);

impl<G: traversal::Graph<NodeId, Edge>> traversal::Graph<EqNode, EqEdge> for EqNeqFilter<G> {
    fn outgoing(&self, node: EqNode) -> impl Iterator<Item = EqEdge> {
        self.0.outgoing(node.0).filter_map(move |e| {
            let r = (e.relation + node.1)?;
            Some(EqEdge(e, r))
        })
    }
}

pub trait EqNeqExt<G: traversal::Graph<NodeId, Edge>> {
    /// Transform the graph in order to traverse it following equality's transitivity laws.
    ///
    /// Modifies the graph so that each node has two copies: One with Eq relation, and one with Neq relation.
    ///
    /// Adapts edges so that a -=> b && b -!=-> c, a -!=-> c and so on.
    fn eq_neq(self) -> EqNeqFilter<G>;
}
impl<G> EqNeqExt<G> for G
where
    G: traversal::Graph<NodeId, Edge>,
{
    fn eq_neq(self) -> EqNeqFilter<G> {
        EqNeqFilter(self)
    }
}

/// Filter the graph according to a closure.
pub struct FilteredGraph<N, E, G, F>(G, F, std::marker::PhantomData<(N, E)>)
where
    N: traversal::Node,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool;

impl<N, E, G, F> traversal::Graph<N, E> for FilteredGraph<N, E, G, F>
where
    N: traversal::Node,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool,
{
    fn outgoing(&self, node: N) -> impl Iterator<Item = E> {
        self.0.outgoing(node).filter(move |e| self.1(node, e))
    }
}

pub trait FilterExt<N, E, G, F>
where
    N: traversal::Node,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool,
{
    /// Filter the graph according to a closure.
    fn filter(self, f: F) -> FilteredGraph<N, E, G, F>;
}
impl<N, E, G, F> FilterExt<N, E, G, F> for G
where
    N: traversal::Node,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool,
{
    fn filter(self, f: F) -> FilteredGraph<N, E, G, F> {
        FilteredGraph(self, f, std::marker::PhantomData {})
    }
}
