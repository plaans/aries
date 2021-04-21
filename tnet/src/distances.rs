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
pub struct DijkstraState {
    latest: BoundValueAdd,
    pub distances: RefMap<VarBound, BoundValueAdd>,
    queue: BinaryHeap<HeapElem>,
}

impl DijkstraState {
    pub fn clear(&mut self) {
        self.latest = BoundValueAdd::ZERO;
        self.distances.clear();
        self.queue.clear()
    }

    pub fn enqueue(&mut self, node: VarBound, dist: BoundValueAdd) {
        if dist < self.distances.get(node).copied().unwrap_or(BoundValueAdd::MAX) {
            self.distances.insert(node, dist);
            self.queue.push(HeapElem { dist, node });
        }
    }

    pub fn dequeue(&mut self) -> Option<(VarBound, BoundValueAdd)> {
        match self.queue.pop() {
            Some(e) => {
                debug_assert!(self.latest <= e.dist);
                debug_assert!(self.distances[e.node] <= e.dist);
                self.latest = e.dist;
                if self.distances[e.node] == e.dist {
                    Some((e.node, e.dist))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn is_final(&self, node: VarBound) -> bool {
        match self.distances.get(node) {
            Some(d) => d <= &self.latest,
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
