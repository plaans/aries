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
    use itertools::Itertools;
    use rand::prelude::SeedableRng;
    use rand::prelude::SmallRng;
    use rand::Rng;
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashMap, HashSet};
    use std::iter::once;

    struct Reverse<'a, G: Graph>(&'a G);

    impl<'a, G: Graph> Graph for Reverse<'a, G> {
        fn vertices(&self) -> impl Iterator<Item = V> + '_ {
            self.0.vertices()
        }

        fn outgoing(&self, src: V) -> impl Iterator<Item = Edge> + '_ {
            self.0.incoming(src).map(|e| Edge::new(e.tgt, e.src, e.weight))
        }

        fn incoming(&self, src: V) -> impl Iterator<Item = Edge> + '_ {
            self.0.outgoing(src).map(|e| Edge::new(e.tgt, e.src, e.weight))
        }
    }

    pub trait Graph {
        fn vertices(&self) -> impl Iterator<Item = V> + '_;
        fn outgoing(&self, src: V) -> impl Iterator<Item = Edge> + '_;
        fn incoming(&self, src: V) -> impl Iterator<Item = Edge> + '_;

        fn relevants(&self, new_edge: &Edge) -> Vec<(V, IntCst)> {
            dbg!(new_edge);
            let mut relevants = Vec::new();
            let mut visited = HashSet::new();
            let mut heap = BinaryHeap::new();

            let mut best_label: HashMap<V, Label> = HashMap::new();

            // order allows to override the label of the target edge if the edge is a self loop
            let tgt_lbl = Label::new(new_edge.weight, true);
            best_label.insert(new_edge.tgt, tgt_lbl);
            heap.push((tgt_lbl, new_edge.tgt));
            let src_lbl = Label::new(0, false);
            best_label.insert(new_edge.src, src_lbl);
            heap.push((src_lbl, new_edge.src));

            // count of the number of unvisited relevants in the queue
            let mut remaining_relevants: u32 = 1;

            while let Some((lbl @ Label { dist, relevant }, curr)) = heap.pop() {
                if visited.contains(&curr) {
                    // already treated, ignore
                    continue;
                }
                visited.insert(curr);
                debug_assert_eq!(lbl, best_label[&curr]);
                if relevant {
                    // there is a shortest path through new edge to v
                    relevants.push((curr, dist));
                    remaining_relevants -= 1;
                }
                for out in self.outgoing(curr) {
                    let lbl = Label::new(dist + out.weight, relevant);

                    if let Some(previous_label) = best_label.get(&out.tgt) {
                        if previous_label >= &lbl {
                            debug_assert!(previous_label.dist <= lbl.dist);
                            continue; // no improvement, ignore
                        }
                        if previous_label.relevant && !lbl.relevant {
                            remaining_relevants -= 1
                        } else if !previous_label.relevant && lbl.relevant {
                            remaining_relevants += 1;
                        }
                    } else if lbl.relevant {
                        remaining_relevants += 1;
                    }
                    best_label.insert(out.tgt, lbl);
                    heap.push((lbl, out.tgt));
                }
                if remaining_relevants == 0 {
                    // there is no hope of finding new relevants;
                    break;
                }
            }

            relevants
        }

        fn potentially_updated_paths(&self, new_edge: &Edge) -> Vec<Edge>
        where
            Self: Sized,
        {
            let mut updated_paths = Vec::with_capacity(32);
            let relevants_after = self.relevants(new_edge);
            let reversed = Reverse(self);
            let relevants_before = reversed.relevants(&new_edge.reverse());

            for (end, cost_from_src) in relevants_after {
                for (orig, cost_to_tgt) in relevants_before.iter().copied() {
                    updated_paths.push(Edge {
                        src: orig,
                        tgt: end,
                        weight: cost_to_tgt - new_edge.weight + cost_from_src,
                    })
                }
            }
            updated_paths
        }

        fn ssp(&self, src: V, tgt: V) -> Option<IntCst> {
            let mut visited = HashSet::new();
            // this is a max heap, so we will store the negation of computed distances
            let mut heap = BinaryHeap::new();

            heap.push((-0, src));

            while let Some((neg_dist, curr)) = heap.pop() {
                if visited.contains(&curr) {
                    // already treated, ignore
                    continue;
                }
                visited.insert(curr);
                if curr == tgt {
                    return Some(-neg_dist);
                }
                for out in self.outgoing(curr) {
                    let lbl = neg_dist - out.weight;
                    heap.push((lbl, out.tgt));
                }
            }
            None
        }
    }

    #[derive(Eq, PartialEq, Copy, Clone, Debug)]
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

    #[derive(Debug, Copy, Clone)]
    struct Edge {
        src: V,
        tgt: V,
        weight: L,
    }

    impl Edge {
        pub fn new(src: V, tgt: V, label: L) -> Self {
            Self {
                src,
                tgt,
                weight: label,
            }
        }
        pub fn reverse(self) -> Self {
            Self {
                src: self.tgt,
                tgt: self.src,
                weight: self.weight,
            }
        }
    }

    impl Graph for &[Edge] {
        fn vertices(&self) -> impl Iterator<Item = V> + '_ {
            self.iter()
                .flat_map(|e| once(e.src).chain(once(e.tgt)))
                .sorted()
                .unique()
        }
        fn outgoing(&self, v: V) -> impl Iterator<Item = Edge> + '_ {
            self.iter().copied().filter(move |e| e.src == v)
        }
        fn incoming(&self, v: V) -> impl Iterator<Item = Edge> + '_ {
            self.iter().copied().filter(move |e| e.tgt == v)
        }
    }

    fn gen_graph(seed: u64) -> Vec<Edge> {
        let mut graph = Vec::new();
        let mut rng = SmallRng::seed_from_u64(seed);
        let num_vertices = rng.gen_range(4..5);
        let num_edges = rng.gen_range(2..=6);

        for _ in 0..num_edges {
            let src = rng.gen_range(0..num_vertices);
            let tgt = rng.gen_range(0..num_vertices);
            let weight = rng.gen_range(0..10);
            let edge = Edge { src, tgt, weight };
            graph.push(edge)
        }

        graph
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

        assert_eq!(g.ssp(1, 2), Some(1));
        assert_eq!(g.ssp(1, 3), Some(4));
        assert_eq!(g.ssp(1, 4), Some(2));

        let graphs = (0..1000).map(gen_graph).collect_vec();

        for graph in graphs {
            let original_graph = &graph[1..];
            let added_edge = &graph[0];
            let final_graph = graph.as_slice();

            let updated = original_graph.relevants(added_edge);
            let updated: HashMap<V, IntCst> = updated.into_iter().collect();
            println!("{:?}", final_graph);
            dbg!("{:?}", updated.clone());

            for other in final_graph.vertices() {
                let previous = original_graph.ssp(added_edge.src, other);
                let new = final_graph.ssp(added_edge.src, other);
                let new_sp = match (previous, new) {
                    (Some(previous), Some(new)) => new < previous,
                    (None, Some(_new)) => true,
                    (Some(_), None) => panic!("A path disapeared ?"),
                    _ => false,
                };
                let present_in_updated = updated.contains_key(&other);
                assert_eq!(new_sp, present_in_updated, "{:?} -> {:?}", added_edge.src, other);
                if present_in_updated {
                    assert_eq!(
                        updated[&other],
                        new.unwrap(),
                        "The length of the shortest paths should be the same"
                    );
                }
            }
        }

        // assert_eq!(relevants(&g[1..=3], &g[0]), vec! {2});
        // assert_eq!(relevants(&g[1..=4], &g[0]), vec! {2, 4});
    }

    #[test]
    fn test_graph_updates() {
        let graphs = (0..1000).map(gen_graph).collect_vec();

        for graph in graphs {
            let original_graph = &graph[1..];
            let added_edge = &graph[0];
            let final_graph = graph.as_slice();

            let updated_paths = original_graph.potentially_updated_paths(added_edge);
            let updated_paths: HashMap<(V, V), IntCst> =
                updated_paths.into_iter().map(|e| ((e.src, e.tgt), e.weight)).collect();

            for orig in final_graph.vertices() {
                for dest in final_graph.vertices() {
                    let previous = original_graph.ssp(orig, dest);
                    let new = final_graph.ssp(orig, dest);
                    let new_sp = match (previous, new) {
                        (Some(previous), Some(new)) => new < previous,
                        (None, Some(_new)) => true,
                        (Some(_), None) => panic!("A path disapeared ?"),
                        _ => false,
                    };
                    let present_in_updated = updated_paths.contains_key(&(orig, dest));
                    assert!(!new_sp || present_in_updated); // new_sp => present_in_updated
                }
            }
        }

        // assert_eq!(relevants(&g[1..=3], &g[0]), vec! {2});
        // assert_eq!(relevants(&g[1..=4], &g[0]), vec! {2, 4});
    }
}
