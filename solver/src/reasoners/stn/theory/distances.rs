use crate::collections::ref_store::RefMap;
use crate::core::*;
use crate::reasoners::stn::theory::PropagatorId;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// An element is the heap: composed of a node and the reduced distance from this origin to this
/// node.
/// We implement the Ord/PartialOrd trait so that a max-heap would return the element with the
/// smallest reduced distance first.
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct HeapElem {
    dist: BoundValueAdd,
    node: SignedVar,
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
    pub distances: RefMap<SignedVar, (BoundValueAdd, Option<PropagatorId>)>,
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
    pub fn enqueue(&mut self, node: SignedVar, dist: BoundValueAdd, incoming_edge: Option<PropagatorId>) {
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
    pub fn dequeue(&mut self) -> Option<(SignedVar, BoundValueAdd)> {
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
    pub fn distance(&self, node: SignedVar) -> Option<BoundValueAdd> {
        self.distances.get(node).map(|(dist, _)| *dist)
    }

    /// Returns an iterator over all nodes (and their distances from the origin) that were reached
    /// by the algorithm.
    pub fn distances(&self) -> impl Iterator<Item = (SignedVar, BoundValueAdd)> + '_ {
        self.distances.entries().map(|(node, (dist, _))| (node, *dist))
    }

    /// Return the predecessor edge from the origin to this node or None if it is an origin.
    ///
    /// **Panics** if the node has no associated distance (i.e. was not reached by the algorithm).
    pub fn predecessor(&self, node: SignedVar) -> Option<PropagatorId> {
        self.distances[node].1
    }

    /// Returns true if the node has a distance that is guaranteed not to change
    /// in subsequent iterations.
    pub fn is_final(&self, node: SignedVar) -> bool {
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

#[cfg(test)]
mod test {
    use crate::core::IntCst;
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashSet};

    #[derive(Eq, PartialEq)]
    struct Label {
        dist: IntCst,
        relevant: bool,
    }

    impl Label {
        pub fn new(dist: IntCst, relevant: bool) -> Self {
            Self { dist, relevant }
        }
    }

    impl PartialOrd<Self> for Label {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Label {
        fn cmp(&self, other: &Self) -> Ordering {
            // ordering compatible with a max heap, giving the priority of the node
            match self.dist.cmp(&other.dist) {
                Ordering::Less => Ordering::Greater,
                Ordering::Equal => {
                    if self.relevant == other.relevant {
                        Ordering::Equal
                    } else if self.relevant {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
                Ordering::Greater => Ordering::Less,
            }
        }
    }

    type V = u32;
    type L = IntCst;

    struct Edge {
        src: V,
        tgt: V,
        label: L,
    }

    impl Edge {
        pub fn new(src: V, tgt: V, label: L) -> Self {
            Self { src, tgt, label }
        }
    }

    fn succs(edges: &[Edge], src: V) -> impl Iterator<Item = &Edge> + '_ {
        edges.iter().filter(move |e| e.src == src)
    }

    fn relevants(g: &[Edge], new_edge: &Edge) -> Vec<V> {
        let mut relevants = Vec::new();
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();

        heap.push((Label::new(0, false), new_edge.src));
        heap.push((Label::new(new_edge.label, true), new_edge.tgt));

        while let Some((Label { dist, relevant }, curr)) = heap.pop() {
            if visited.contains(&curr) {
                // already treated, ignore
                continue;
            }
            visited.insert(curr);
            if relevant {
                // there is a shortest path through new edge to v
                relevants.push(curr)
            }
            for out in succs(g, curr) {
                let lbl = Label::new(dist + out.label, relevant);
                heap.push((lbl, out.tgt));
            }
        }

        relevants
    }

    fn ssp(g: &[Edge], src: V, tgt: V) -> Option<IntCst> {
        // this is a max heap, so we will store the negation of computed distances
        let mut heap = BinaryHeap::new();

        heap.push((-0, src));

        while let Some((neg_dist, curr)) = heap.pop() {
            if curr == tgt {
                return Some(-neg_dist);
            }
            for out in succs(g, curr) {
                let lbl = neg_dist - out.label;
                heap.push((lbl, out.tgt));
            }
        }
        None
    }

    #[test]
    fn test_graph() {
        let g: &[Edge] = &[
            Edge::new(1, 2, 1),
            Edge::new(1, 2, 2),
            Edge::new(1, 3, 4),
            Edge::new(1, 4, 5),
            Edge::new(2, 4, 1),
        ];

        assert_eq!(ssp(g, 1, 2), Some(1));
        assert_eq!(ssp(g, 1, 3), Some(4));
        assert_eq!(ssp(g, 1, 4), Some(2));

        let graphs = vec![g];

        for graph in graphs {
            let original_graph = &graph[1..];
            let added_edge = &graph[0];
            let final_graph = graph;
            let updated = relevants(original_graph, added_edge);

            for up in updated {
                let previous = ssp(original_graph, added_edge.src, up);
                let new = ssp(final_graph, added_edge.src, up).unwrap();
                println!("{up}: {previous:?} -> {new}");
                assert!(previous.is_none() || previous.unwrap() > new);
            }
        }

        // assert_eq!(relevants(&g[1..=3], &g[0]), vec! {2});
        // assert_eq!(relevants(&g[1..=4], &g[0]), vec! {2, 4});
    }
}
