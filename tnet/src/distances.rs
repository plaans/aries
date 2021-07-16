use crate::theory::DirEdge;
use aries_collections::ref_store::RefMap;
use aries_model::bounds::{BoundValueAdd, VarBound};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// An element is the heap: composed of a node and the reduced distance from this origin to this
/// node.
/// We implement the Ord/PartialOrd trait so that a max-heap would return the element with the
/// smallest reduced distance first.
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct HeapElem {
    dist: BoundValueAdd,
    node: VarBound,
}
impl PartialOrd for HeapElem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapElem {
    fn cmp(&self, other: &Self) -> Ordering {
        Reverse(self.dist).cmp(&Reverse(other.dist))
    }
}

/// A Data structure that contains the mutable data that is updated during a Dijkstra algorithm.
/// It is intended to be reusable across multiple runs.
#[derive(Clone)]
pub(crate) struct DijkstraState {
    /// The latest distance that was extracted from the queue
    /// As a property of the Dijkstra algorithm, if a distance in the `distances` table
    /// is leq to this value, then it will not change for the rest of process.
    latest: BoundValueAdd,
    /// Associates each vertex to its distance.
    /// If the node is not an origin, it also indicates the latest edge on the shortest path to this node.
    pub distances: RefMap<VarBound, (BoundValueAdd, Option<DirEdge>)>,
    /// Elements of the queue that have not been extracted yet.
    /// Note that a single node might appear several times in the queue, in which case only
    /// the one with the smallest distance is relevant.
    queue: BinaryHeap<HeapElem>,
}

impl DijkstraState {
    pub fn clear(&mut self) {
        self.latest = BoundValueAdd::ZERO;
        self.distances.clear();
        self.queue.clear()
    }

    /// Add a node to the queue, indicating the distance from the origin and the latest edge
    /// on the path from the origin to this node.
    pub fn enqueue(&mut self, node: VarBound, dist: BoundValueAdd, incoming_edge: Option<DirEdge>) {
        let previous_dist = match self.distances.get(node) {
            None => BoundValueAdd::MAX,
            Some((prev, _)) => *prev,
        };
        if dist < previous_dist {
            self.distances.insert(node, (dist, incoming_edge));
            self.queue.push(HeapElem { dist, node });
        }
    }

    /// Remove the next element in the queue.
    /// Nodes are removed by increasing distance to the origin.
    /// Each node can only be extracted once.
    pub fn dequeue(&mut self) -> Option<(VarBound, BoundValueAdd)> {
        match self.queue.pop() {
            Some(e) => {
                debug_assert!(self.latest <= e.dist);
                debug_assert!(self.distances[e.node].0 <= e.dist);
                self.latest = e.dist;
                if self.distances[e.node].0 == e.dist {
                    // node with the best distance from origin
                    Some((e.node, e.dist))
                } else {
                    // a node with a better distance was previously extracted, ignore this one
                    // as it can not contribute to a shortest path
                    None
                }
            }
            None => None,
        }
    }

    /// Returns the distance from the origin to this node, or `None` if the node was not reached
    /// by the Dijkstra algorithm.
    pub fn distance(&self, node: VarBound) -> Option<BoundValueAdd> {
        self.distances.get(node).map(|(dist, _)| *dist)
    }

    /// Returns an iterator over all nodes (and their distances from the origin) that were reached
    /// by the algorithm.
    pub fn distances(&self) -> impl Iterator<Item = (VarBound, BoundValueAdd)> + '_ {
        self.distances.entries().map(|(node, (dist, _))| (node, *dist))
    }

    /// Return the predecessor edge from the origin to this node or None if it is an origin.
    ///
    /// **Panics** if the node has no associated distance (i.e. was not reached by the algorithm).
    pub fn predecessor(&self, node: VarBound) -> Option<DirEdge> {
        self.distances[node].1
    }

    /// Returns true if the node has a distance that is guaranteed not to change
    /// in subsequent iterations.
    pub fn is_final(&self, node: VarBound) -> bool {
        match self.distances.get(node) {
            Some((d, _)) => d <= &self.latest,
            None => false,
        }
    }
}

impl Default for DijkstraState {
    fn default() -> Self {
        DijkstraState {
            latest: BoundValueAdd::ZERO,
            distances: Default::default(),
            queue: Default::default(),
        }
    }
}
