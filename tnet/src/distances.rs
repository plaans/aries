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
#[derive(Default, Clone)]
pub struct DijkstraState {
    pub distances: RefMap<VarBound, BoundValueAdd>,
    queue: BinaryHeap<HeapElem>,
}

impl DijkstraState {
    pub fn clear(&mut self) {
        self.distances.clear();
        self.queue.clear()
    }

    pub fn enqueue(&mut self, node: VarBound, dist: BoundValueAdd) {
        self.queue.push(HeapElem { dist, node });
    }

    pub fn dequeue(&mut self) -> Option<(VarBound, BoundValueAdd)> {
        self.queue.pop().map(|e| (e.node, e.dist))
    }
}
