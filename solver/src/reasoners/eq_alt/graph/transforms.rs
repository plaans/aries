use crate::{collections::ref_store::Ref, reasoners::eq_alt::relation::EqRelation};

use super::{
    traversal::{self},
    EqAdjList, IdEdge, NodeId, Path,
};

// Implementations of generic edge for concrete edge type
impl traversal::Edge<NodeId> for IdEdge {
    fn target(&self) -> NodeId {
        self.target
    }

    fn source(&self) -> NodeId {
        self.source
    }
}

// Implementation of generic graph for concrete graph
impl traversal::Graph<NodeId, IdEdge> for &EqAdjList {
    fn outgoing(&self, node: NodeId) -> impl Iterator<Item = IdEdge> {
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

// T gets first bit, N is shifted by one
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

/// Second field is the relation of the target node
/// (Hence the - in source)
#[derive(Debug, Clone)]
pub struct EqEdge(pub IdEdge, EqRelation);

impl traversal::Edge<EqNode> for EqEdge {
    fn target(&self) -> EqNode {
        EqNode(self.0.target, self.1)
    }

    fn source(&self) -> EqNode {
        EqNode(self.0.source, (self.1 - self.0.relation).unwrap())
    }
}

/// Filters the traversal to only include Eq
pub struct EqFilter<G: traversal::Graph<NodeId, IdEdge>>(G);

impl<G: traversal::Graph<NodeId, IdEdge>> traversal::Graph<NodeId, IdEdge> for EqFilter<G> {
    fn outgoing(&self, node: NodeId) -> impl Iterator<Item = IdEdge> {
        self.0.outgoing(node).filter(|e| e.relation == EqRelation::Eq)
    }
}

pub trait EqExt<G: traversal::Graph<NodeId, IdEdge>> {
    fn eq(self) -> EqFilter<G>;
}
impl<G> EqExt<G> for G
where
    G: traversal::Graph<NodeId, IdEdge>,
{
    fn eq(self) -> EqFilter<G> {
        EqFilter(self)
    }
}

pub struct EqNeqFilter<G: traversal::Graph<NodeId, IdEdge>>(G);

impl<G: traversal::Graph<NodeId, IdEdge>> traversal::Graph<EqNode, EqEdge> for EqNeqFilter<G> {
    fn outgoing(&self, node: EqNode) -> impl Iterator<Item = EqEdge> {
        self.0.outgoing(node.0).filter_map(move |e| {
            let r = (e.relation + node.1)?;
            Some(EqEdge(e, r))
        })
    }
}

pub trait EqNeqExt<G: traversal::Graph<NodeId, IdEdge>> {
    fn eq_neq(self) -> EqNeqFilter<G>;
}
impl<G> EqNeqExt<G> for G
where
    G: traversal::Graph<NodeId, IdEdge>,
{
    fn eq_neq(self) -> EqNeqFilter<G> {
        EqNeqFilter(self)
    }
}

pub struct FilteredGraph<N, E, G, F>(G, F, std::marker::PhantomData<(N, E)>)
where
    N: Ref,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool;

impl<N, E, G, F> traversal::Graph<N, E> for FilteredGraph<N, E, G, F>
where
    N: Ref,
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
    N: Ref,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool,
{
    fn filter(self, f: F) -> FilteredGraph<N, E, G, F>;
}
impl<N, E, G, F> FilterExt<N, E, G, F> for G
where
    N: Ref,
    E: traversal::Edge<N>,
    G: traversal::Graph<N, E>,
    F: Fn(N, &E) -> bool,
{
    fn filter(self, f: F) -> FilteredGraph<N, E, G, F> {
        FilteredGraph(self, f, std::marker::PhantomData {})
    }
}
