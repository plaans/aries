use state::{Domains, DomainsSnapshot};

use crate::backtrack::EventIndex;
use crate::core::*;
use crate::reasoners::stn::theory::PropagatorId;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use super::StnTheory;

struct Reversed<'a, V: Copy, E: Copy, G: Graph<V, E>>(&'a G, PhantomData<V>, PhantomData<E>);

impl<'a, V: Copy, E: Copy, G: Graph<V, E>> Graph<V, E> for Reversed<'a, V, E, G> {
    fn vertices(&self) -> impl Iterator<Item = V> + '_ {
        self.0.vertices()
    }

    fn edge(&self, e: E) -> Edge<V, E> {
        let edge = self.0.edge(e);
        Edge::new(edge.src, edge.tgt, edge.weight, e)
    }

    fn outgoing(&self, src: V) -> impl Iterator<Item = Edge<V, E>> + '_ {
        self.0.incoming(src).map(|e| Edge::new(e.tgt, e.src, e.weight, e.id))
    }

    fn incoming(&self, src: V) -> impl Iterator<Item = Edge<V, E>> + '_ {
        self.0.outgoing(src).map(|e| Edge::new(e.tgt, e.src, e.weight, e.id))
    }

    fn potential(&self, v: V) -> IntCst {
        -self.0.potential(v)
    }
}

pub trait Graph<V: Copy, E: Copy> {
    fn vertices(&self) -> impl Iterator<Item = V> + '_;
    fn outgoing(&self, v: V) -> impl Iterator<Item = Edge<V, E>> + '_;
    fn incoming(&self, v: V) -> impl Iterator<Item = Edge<V, E>> + '_;

    fn potential(&self, v: V) -> IntCst;

    fn edge(&self, e: E) -> Edge<V, E>;

    fn edges(&self) -> impl Iterator<Item = Edge<V, E>> + '_ {
        self.vertices().flat_map(|v| self.outgoing(v))
    }

    /// Returns a view of the graph with the edges swapped.
    /// The potential function is modified to be valid in the reversed graph.
    fn reversed(&self) -> impl Graph<V, E> + '_
    where
        Self: Sized,
        E: 'static,
        V: 'static,
    {
        Reversed(self, PhantomData, PhantomData)
    }

    /// Returns the set of vertices for which adding the edge `e` would result in a new shortest `src(e) -> v`.
    /// Each vertex is tagged with the distance `tgt(e) -> v`.
    fn relevants(&self, new_edge: &Edge<V, E>) -> Vec<(V, IntCst)>
    where
        V: Ord + Hash,
    {
        let mut relevants = Vec::new();
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();

        let mut best_label: HashMap<V, Label> = HashMap::new();

        // order allows to override the label of the target edge if the edge is a self loop
        let reduced_weight = new_edge.weight + self.potential(new_edge.src) - self.potential(new_edge.tgt);
        let tgt_lbl = Label::new(reduced_weight, true);
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
                // there is a new shortest path through new edge to v
                // dist is the length of the path with reduced cost, convert it to normal distances
                let dist = dist - self.potential(new_edge.src) + self.potential(curr);
                relevants.push((curr, dist - new_edge.weight));
                remaining_relevants -= 1;
            }
            for out in self.outgoing(curr) {
                let reduced_cost = out.weight + self.potential(out.src) - self.potential(out.tgt);
                debug_assert!(reduced_cost >= 0);
                let lbl = Label::new(dist + reduced_cost, relevant);

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

    /// Returns true if the potential function is valid for the set of edges
    /// Mostly intended for debugging.
    #[allow(unused)]
    fn is_potential_valid(&self) -> bool {
        for Edge { src, tgt, weight, .. } in self.edges() {
            if self.potential(src) + weight - self.potential(tgt) < 0 {
                return false;
            }
        }
        true
    }

    /// Returns the distance through the shortest path (if any) between the two vertices.
    #[allow(unused)]
    fn shortest_distance(&self, src: V, tgt: V) -> Option<IntCst>
    where
        V: Ord + Hash,
        E: Ord,
    {
        self.ssp(src, tgt).map(|(dist, _preds)| dist)
    }

    /// Returns the (unordered!) set of vertices on a shortest path between the two vertices.
    /// Returns `None` if there is no path.
    fn shortest_path(&self, src: V, tgt: V) -> Option<Vec<E>>
    where
        V: Ord + Hash,
        E: Ord,
    {
        self.ssp(src, tgt).map(|(_dist, preds)| {
            let mut result = Vec::with_capacity(16);
            let mut last = tgt;
            while let Some(inc) = preds.get(last) {
                result.push(inc);
                let edge = self.edge(inc);
                last = edge.src;
            }
            result
        })
    }

    /// Returns the cost of the shortest path between the two vertices, along with the map of the predecessors that
    /// allows reconstructing the shortest path.
    /// Returns `None` if there is no path between the two vertices.
    fn ssp(&self, src: V, tgt: V) -> Option<(IntCst, Predecessors<V, E>)>
    where
        V: Ord + Hash,
        E: Ord,
    {
        let mut preds = Predecessors::default();
        // this is a max heap, so we will store the negation of computed distances
        let mut heap = BinaryHeap::new();

        heap.push((-0, src, None));

        while let Some((neg_dist, curr, pred)) = heap.pop() {
            if preds.is_set(curr) {
                // already treated, ignore
                continue;
            }
            preds.set(curr, pred);
            if curr == tgt {
                let reduced_dist = -neg_dist;
                let dist = reduced_dist - self.potential(src) + self.potential(tgt);
                return Some((dist, preds));
            }
            for out in self.outgoing(curr) {
                let reduced_cost = self.potential(out.src) + out.weight - self.potential(out.tgt);
                debug_assert!(reduced_cost >= 0);
                let lbl = neg_dist - reduced_cost;
                heap.push((lbl, out.tgt, Some(out.id)));
            }
        }
        None
    }

    /// Returns a caracterisation of the set of paths that would be updated/appear if the given edge was to be added to the graph.
    /// Those consist of:
    ///   - paths whose last edge is the additional one (prefix paths)
    ///   - path whose first edge is the additional one (postfix paths)
    ///
    /// Note if there is a new shorter path in the graph after the addition of the edge, it must be composed of:
    ///  - a (possibly empty) prefix path
    ///  - the additional edge
    ///  - a (possibly empty) postfix path
    fn updated_on_addition(&self, source: V, target: V, weight: IntCst, id: E) -> PotentialUpdate<V>
    where
        V: Hash + Ord + 'static,
        E: 'static,
        Self: Sized,
    {
        let new_edge = &Edge::new(source, target, weight, id);

        let relevants_after = self.relevants(new_edge);
        let postfixes = relevants_after.into_iter().collect();

        let reversed = self.reversed();
        let relevants_before = reversed.relevants(&new_edge.reverse());
        let prefixes = relevants_before.into_iter().collect();

        PotentialUpdate { prefixes, postfixes }
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

type L = IntCst;

#[derive(Debug, Copy, Clone)]
pub struct Edge<V: Copy, E: Copy> {
    src: V,
    tgt: V,
    weight: L,
    id: E,
}

impl<V: Copy, E: Copy> Edge<V, E> {
    pub fn new(src: V, tgt: V, label: L, id: E) -> Self {
        Self {
            src,
            tgt,
            weight: label,
            id,
        }
    }
    pub fn reverse(self) -> Self {
        Self {
            src: self.tgt,
            tgt: self.src,
            weight: self.weight,
            id: self.id,
        }
    }
}

/// Characterisation of the potential updates in the shortest paths following the addition of an edge `e`
pub struct PotentialUpdate<V: Hash> {
    /// All nodes `v` for which the addition of `e` results in a new shortest path `v -> tgt(e)`
    /// It is annotated with the distance `v -> src(e)`
    pub prefixes: HashMap<V, IntCst>,
    /// All nodes `v` for which the addition of `e` results in a new shortest path `src(e) -> v`
    /// It is annotated with the distance `tgt(e) -> v`
    pub postfixes: HashMap<V, IntCst>,
}

pub struct Predecessors<V, E> {
    preds: HashMap<V, Option<E>>,
}
impl<V, E> Default for Predecessors<V, E> {
    fn default() -> Self {
        Self { preds: HashMap::new() }
    }
}
impl<V: Hash + Eq + Copy, E: Copy> Predecessors<V, E> {
    pub fn set(&mut self, v: V, e: Option<E>) {
        debug_assert!(!self.is_set(v));
        self.preds.insert(v, e);
    }

    pub fn is_set(&self, v: V) -> bool {
        self.preds.contains_key(&v)
    }

    pub fn get(&self, v: V) -> Option<E> {
        self.preds[&v]
    }
}

/// View of an STN as a graph, possibly ommiting a particular edge.
/// It uses the upper bounds as the potential function which limits its usage to cases where the upper bounds are fully propagated
pub struct StnGraph<'a> {
    stn: &'a StnTheory,
    doms: &'a Domains,
    /// If set, the graph will not contain the marked edge (used to do as if the edge that we want to study was not in the grpah yet)
    ignored: Option<PropagatorId>,
}

impl<'a> StnGraph<'a> {
    #[allow(unused)]
    pub fn new(stn: &'a StnTheory, doms: &'a Domains) -> Self {
        Self {
            stn,
            doms,
            ignored: None,
        }
    }

    pub fn new_excluding(stn: &'a StnTheory, doms: &'a Domains, excluded: PropagatorId) -> Self {
        Self {
            stn,
            doms,
            ignored: Some(excluded),
        }
    }
}
pub type StnEdge = Edge<SignedVar, PropagatorId>;

impl<'a> Graph<SignedVar, PropagatorId> for StnGraph<'a> {
    fn vertices(&self) -> impl Iterator<Item = SignedVar> + '_ {
        self.stn.active_propagators.keys()
    }

    fn edge(&self, e: PropagatorId) -> Edge<SignedVar, PropagatorId> {
        let prop = &self.stn.constraints[e];
        Edge::new(prop.source, prop.target, prop.weight, e)
    }

    fn outgoing(&self, v: SignedVar) -> impl Iterator<Item = StnEdge> + '_ {
        self.stn.active_propagators[v]
            .iter()
            .filter(|prop| self.ignored != Some(prop.id))
            .filter(|prop| self.doms.present(prop.target) != Some(false))
            .map(move |prop| StnEdge {
                src: v,
                tgt: prop.target,
                weight: prop.weight,
                id: prop.id,
            })
    }

    fn incoming(&self, v: SignedVar) -> impl Iterator<Item = StnEdge> + '_ {
        self.stn.incoming_active_propagators[v]
            .iter()
            .filter(|prop| self.ignored != Some(prop.id))
            .filter(|prop| self.doms.present(prop.target) != Some(false))
            .map(move |prop| StnEdge {
                src: prop.target,
                tgt: v,
                weight: prop.weight,
                id: prop.id,
            })
    }

    fn potential(&self, v: SignedVar) -> IntCst {
        // if the domains have been fully propagated, then the upper bounds constitute a valid potential function
        self.doms.ub(v)
    }
}

/// View of an STN as a graph, at an older point in time.
pub struct StnSnapshotGraph<'a> {
    stn: &'a StnTheory,
    /// Representation of the domains at some point in past
    doms: &'a DomainsSnapshot<'a>,
    /// All edges that were inserted after this event (in the grpah edge insertion trail) should be ignored
    ignore_after: EventIndex,
}

impl<'a> StnSnapshotGraph<'a> {
    pub fn new(stn: &'a StnTheory, doms: &'a DomainsSnapshot<'a>, ignore_after: EventIndex) -> Self {
        Self {
            stn,
            doms,
            ignore_after,
        }
    }
}

impl<'a> Graph<SignedVar, PropagatorId> for StnSnapshotGraph<'a> {
    fn vertices(&self) -> impl Iterator<Item = SignedVar> + '_ {
        self.stn.active_propagators.keys()
    }

    fn edge(&self, e: PropagatorId) -> Edge<SignedVar, PropagatorId> {
        let prop = &self.stn.constraints[e];
        Edge::new(prop.source, prop.target, prop.weight, e)
    }

    fn outgoing(&self, v: SignedVar) -> impl Iterator<Item = StnEdge> + '_ {
        self.stn.active_propagators[v]
            .iter()
            .filter(|prop| self.doms.present(prop.target) != Some(false))
            .filter(|prop| {
                // we are considering the view of an older STN, thus we must ignore any
                // edge that was not active according to the domains at the time (if the edge has been added to the STN since)
                let c = &self.stn.constraints[prop.id];
                let enabler = c.enabler.unwrap().1;
                enabler < self.ignore_after
            })
            .map(move |prop| StnEdge {
                src: v,
                tgt: prop.target,
                weight: prop.weight,
                id: prop.id,
            })
    }

    fn incoming(&self, v: SignedVar) -> impl Iterator<Item = StnEdge> + '_ {
        self.stn.incoming_active_propagators[v]
            .iter()
            .filter(|prop| self.doms.present(prop.target) != Some(false))
            .filter(|prop| {
                // we are considering the view of an older STN, thus we ignore any edge inserted after our timestamp
                let c = &self.stn.constraints[prop.id];
                let enabler = c.enabler.unwrap().1;
                enabler < self.ignore_after
            })
            .map(move |prop| StnEdge {
                src: prop.target,
                tgt: v,
                weight: prop.weight,
                id: prop.id,
            })
    }

    fn potential(&self, v: SignedVar) -> IntCst {
        self.doms.ub(v)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::IntCst;
    use itertools::Itertools;
    use rand::prelude::SeedableRng;
    use rand::prelude::SmallRng;
    use rand::Rng;
    use std::collections::HashMap;
    use std::iter::once;

    type V = u32;

    #[derive(Clone, Copy, Debug)]
    struct TestEdge {
        src: V,
        tgt: V,
        weight: i32,
    }
    impl TestEdge {
        pub fn new(src: V, tgt: V, weight: i32) -> Self {
            Self { src, tgt, weight }
        }
    }

    #[derive(Clone)]
    struct EdgeList {
        edges: Vec<TestEdge>,
        potential: HashMap<V, IntCst>,
    }

    impl EdgeList {
        pub fn new(edges: Vec<TestEdge>) -> Option<Self> {
            potential(&edges).map(|pot| Self { edges, potential: pot })
        }

        pub fn pop_edge(&self) -> (Edge<V, usize>, EdgeList) {
            let mut smaller = self.clone();
            let edge = smaller.edges.pop().unwrap();
            let edge = Edge {
                src: edge.src,
                tgt: edge.tgt,
                weight: edge.weight,
                id: smaller.edges.len(),
            };
            (edge, smaller)
        }
    }

    fn has_negative_cycle(edges: &[TestEdge]) -> bool {
        potential(edges).is_none()
    }

    /// Creates a potential function for the set of edges.
    /// Returns `None` if the graph has negative cycle (i.e. no potential function exists).
    fn potential(edges: &[TestEdge]) -> Option<HashMap<V, IntCst>> {
        let mut potential = HashMap::with_capacity(32);

        // initialization of Bellman-Ford, simulating the presence of a virtual node that has an edge of weight 0 to all vertices
        // after a single iteration, all vertices would have a distance from it of 0
        for e in edges {
            potential.insert(e.src, 0);
            potential.insert(e.tgt, 0);
        }
        let num_vertices = potential.len();
        let mut num_iters = 0;
        let mut update_in_iter = true;
        while update_in_iter {
            num_iters += 1;
            if num_iters == num_vertices + 2 {
                // the N +1 iteration produced a change, we have a negative cycle
                return None;
            }
            update_in_iter = false;
            for e in edges {
                let prev = potential[&e.tgt];
                let update = potential[&e.src] + e.weight;
                if update < prev {
                    potential.insert(e.tgt, update);
                    // at least one change, we must do another iteration
                    update_in_iter = true;
                }
            }
        }
        for e in edges {
            debug_assert!(
                e.weight >= potential[&e.tgt] - potential[&e.src],
                "BUG: Invalid potential function"
            );
        }

        Some(potential)
    }

    impl Graph<V, usize> for EdgeList {
        fn vertices(&self) -> impl Iterator<Item = V> + '_ {
            self.edges
                .iter()
                .flat_map(|e| once(e.src).chain(once(e.tgt)))
                .sorted()
                .unique()
        }
        fn edge(&self, e: usize) -> Edge<V, usize> {
            let edge = self.edges[e];
            Edge::new(edge.src, edge.tgt, edge.weight, e)
        }
        fn outgoing(&self, v: V) -> impl Iterator<Item = Edge<V, usize>> + '_ {
            self.edges
                .iter()
                .enumerate()
                .filter(move |&(_id, e)| e.src == v)
                .map(|(id, e)| Edge::new(e.src, e.tgt, e.weight, id))
        }
        fn incoming(&self, v: V) -> impl Iterator<Item = Edge<V, usize>> + '_ {
            self.edges
                .iter()
                .enumerate()
                .filter(move |&(_id, e)| e.tgt == v)
                .map(|(id, e)| Edge::new(e.src, e.tgt, e.weight, id))
        }

        fn potential(&self, v: V) -> IntCst {
            self.potential[&v]
        }
    }

    /// Generates a random graph from this seed
    fn gen_graph(seed: u64) -> EdgeList {
        let mut graph = Vec::new();
        let mut rng = SmallRng::seed_from_u64(seed);
        let num_vertices = rng.gen_range(4..10);
        let num_edges = rng.gen_range(2..=15);

        while graph.len() < num_edges {
            let src = rng.gen_range(0..num_vertices);
            let tgt = rng.gen_range(0..num_vertices);
            let weight = rng.gen_range(-10..=10);
            let edge = TestEdge { src, tgt, weight };
            graph.push(edge);
            if has_negative_cycle(&graph) {
                // we don't want negative cycle, undo and retry with something else at next iter
                graph.pop().unwrap();
            }
        }

        EdgeList::new(graph).unwrap()
    }

    #[test]
    fn test_distances() {
        let g = EdgeList::new(vec![
            TestEdge::new(1, 2, 1),
            TestEdge::new(1, 2, 2),
            TestEdge::new(1, 3, 4),
            TestEdge::new(1, 4, 5),
            TestEdge::new(2, 4, 1),
        ])
        .unwrap();

        assert_eq!(g.shortest_distance(1, 2), Some(1));
        assert_eq!(g.shortest_distance(1, 3), Some(4));
        assert_eq!(g.shortest_distance(1, 4), Some(2));
    }

    #[test]
    fn test_distances_negative() {
        let g = EdgeList::new(vec![
            TestEdge::new(1, 2, 1),
            TestEdge::new(1, 2, -1),
            TestEdge::new(1, 3, 4),
            TestEdge::new(1, 4, 5),
            TestEdge::new(2, 4, 0),
            TestEdge::new(4, 3, 1),
        ])
        .unwrap();

        assert_eq!(g.shortest_distance(1, 2), Some(-1));
        assert_eq!(g.shortest_distance(1, 4), Some(-1));
        assert_eq!(g.shortest_distance(1, 3), Some(0));
    }

    #[test]
    fn test_potentials() {
        // the validity of potential functions is checked with assertions at the end of its construction, just some simple tests for cycle detection

        assert!(!has_negative_cycle(&[
            TestEdge::new(1, 2, 1),
            TestEdge::new(1, 2, 2),
            TestEdge::new(1, 3, 4),
            TestEdge::new(1, 4, 5),
            TestEdge::new(2, 4, 1),
        ]));

        assert!(!has_negative_cycle(&[
            TestEdge::new(1, 2, 1),
            TestEdge::new(2, 1, -1),
            TestEdge::new(1, 3, 4),
            TestEdge::new(1, 4, 5),
            TestEdge::new(2, 4, 1),
        ]));

        assert!(!has_negative_cycle(&[
            TestEdge::new(1, 2, 1),
            TestEdge::new(1, 3, 4),
            TestEdge::new(1, 4, 5),
            TestEdge::new(2, 4, 1),
            TestEdge::new(4, 1, -2),
        ]));

        assert!(has_negative_cycle(&[
            TestEdge::new(1, 2, 1),
            TestEdge::new(1, 3, 4),
            TestEdge::new(1, 4, 5),
            TestEdge::new(2, 4, 1),
            TestEdge::new(4, 1, -3),
        ]));
    }

    /// Test that all relvants nodes for an update are identified.
    #[test]
    fn test_relevance() {
        let graphs = (0..1000).map(gen_graph).collect_vec();

        for final_graph in graphs {
            let (added_edge, original_graph) = final_graph.pop_edge();

            dbg!(&original_graph.edges);
            let updated = original_graph.relevants(&added_edge);
            let updated: HashMap<V, IntCst> = updated.into_iter().collect();

            for other in final_graph.vertices() {
                let previous = original_graph.shortest_distance(added_edge.src, other);
                let new = final_graph.shortest_distance(added_edge.src, other);
                let new_sp = match (previous, new) {
                    (Some(previous), Some(new)) => new < previous,
                    (None, Some(_new)) => true,
                    (Some(_), None) => panic!("A path disappeared ?"),
                    _ => false,
                };
                let present_in_updated = updated.contains_key(&other);
                assert_eq!(new_sp, present_in_updated, "{:?} -> {:?}", added_edge.src, other);
                if present_in_updated {
                    assert_eq!(
                        updated[&other] + added_edge.weight,
                        new.unwrap(),
                        "The length of the shortest paths should be the same  ({} -> {})",
                        added_edge.src,
                        other
                    );
                }
            }
        }
    }

    /// Tests that be do identify a super set of the potentially updated paths.
    #[test]
    fn test_graph_updates() {
        let graphs = (0..1000).map(gen_graph).collect_vec();

        for final_graph in graphs {
            let (added_edge, original_graph) = final_graph.pop_edge();

            let PotentialUpdate { prefixes, postfixes } =
                original_graph.updated_on_addition(added_edge.src, added_edge.tgt, added_edge.weight, added_edge.id);

            let updated_paths: HashMap<(V, V), IntCst> = prefixes
                .into_iter()
                .flat_map(|(orig, orig_src)| {
                    postfixes
                        .iter()
                        .map(move |(dest, tgt_dest)| ((orig, *dest), orig_src + added_edge.weight + tgt_dest))
                })
                .collect();

            for orig in final_graph.vertices() {
                for dest in final_graph.vertices() {
                    let previous = original_graph.shortest_distance(orig, dest);
                    let new = final_graph.shortest_distance(orig, dest);
                    let new_sp = match (previous, new) {
                        (Some(previous), Some(new)) => new < previous,
                        (None, Some(_new)) => true,
                        (Some(_), None) => panic!("A path disappeared ?"),
                        _ => false,
                    };
                    let present_in_updated = updated_paths.contains_key(&(orig, dest));
                    assert!(!new_sp || present_in_updated); // new_sp => present_in_updated
                }
            }
        }
    }

    /// Tests that the sum of edges on the path is the same as the shortest distance
    #[test]
    fn test_graph_path() {
        let graphs = (0..1000).map(gen_graph).collect_vec();

        for graph in graphs {
            let orig = 0;
            let dest = 1;

            let dist = graph.shortest_distance(orig, dest);
            let path = graph.shortest_path(orig, dest);

            if let Some(dist) = dist {
                let path = path.unwrap();
                let path_dist = path.into_iter().map(|e| graph.edge(e).weight).sum();
                assert_eq!(dist, path_dist);
            } else {
                assert!(path.is_none());
            }
        }
    }
}
