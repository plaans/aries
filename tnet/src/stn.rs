use crate::num::Time;
use crate::stn::Event::{EdgeActivated, EdgeAdded, NewPendingActivation, NodeAdded};

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::IndexMut;

/// A timepoint in a Simple Temporal Network.
/// It simply represents a pointer/index into a particular STN object.
/// Creating a timepoint can only be done by invoking the `add_timepoint` method
/// on the STN.
/// It can be converted to an unsigned integer with `to_index` and can be used to
/// index into a Vec.  
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct Timepoint(u32);

impl Timepoint {
    fn from_index(index: u32) -> Timepoint {
        Timepoint(index)
    }

    pub fn to_index(self) -> u32 {
        self.0
    }
}

impl<X> Index<Timepoint> for Vec<X> {
    type Output = X;

    fn index(&self, index: Timepoint) -> &Self::Output {
        &self[index.to_index() as usize]
    }
}

impl<X> IndexMut<Timepoint> for Vec<X> {
    fn index_mut(&mut self, index: Timepoint) -> &mut Self::Output {
        &mut self[index.to_index() as usize]
    }
}

/// A unique identifier for an edge in the STN.
/// An edge and its negation share the same `base_id` but differ by the `is_negated` property.
///
/// For instance, valid edge ids:
///  -  a - b <= 10
///    - base_id: 3
///    - negated: false
///  - a - b > 10       # negation of the previous one
///    - base_id: 3     # same
///    - negated: true  # inverse
///  - a -b <= 20       # unrelated
///    - base_id: 4
///    - negated: false
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct EdgeID {
    base_id: u32,
    negated: bool,
}
impl EdgeID {
    fn new(base_id: u32, negated: bool) -> EdgeID {
        EdgeID { base_id, negated }
    }
    pub fn base_id(&self) -> u32 {
        self.base_id
    }
    pub fn is_negated(&self) -> bool {
        self.negated
    }
}

#[cfg(feature = "theories")]
impl From<EdgeID> for aries_smt::AtomID {
    fn from(edge: EdgeID) -> Self {
        aries_smt::AtomID::new(edge.base_id, edge.negated)
    }
}
#[cfg(feature = "theories")]
impl From<aries_smt::AtomID> for EdgeID {
    fn from(atom: aries_smt::AtomID) -> Self {
        EdgeID::new(atom.base_id(), atom.is_negated())
    }
}

/// An edge in the STN, representing the constraint `target - source <= weight`
/// An edge can be either in canonical form or in negated form.
/// Given to edges (tgt - src <= w) and (tgt -src > w) one will be in canonical form and
/// the other in negated form.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Edge<W> {
    pub source: Timepoint,
    pub target: Timepoint,
    pub weight: W,
}

impl<W: Time> Edge<W> {
    pub fn new(source: Timepoint, target: Timepoint, weight: W) -> Edge<W> {
        Edge { source, target, weight }
    }

    fn is_negated(&self) -> bool {
        !self.is_canonical()
    }

    fn is_canonical(&self) -> bool {
        self.source < self.target || self.source == self.target && self.weight >= W::zero()
    }

    // not(b - a <= 6)
    //   = b - a > 6
    //   = a -b < -6
    //   = a - b <= -7
    //
    // not(a - b <= -7)
    //   = a - b > -7
    //   = b - a < 7
    //   = b - a <= 6
    fn negated(&self) -> Self {
        Edge {
            source: self.target,
            target: self.source,
            weight: -self.weight - W::step(),
        }
    }
}

#[derive(Copy, Clone)]
struct Constraint<W> {
    /// True if the constraint active (participates in propagation)
    active: bool,
    edge: Edge<W>,
}
impl<W> Constraint<W> {
    pub fn new(active: bool, edge: Edge<W>) -> Constraint<W> {
        Constraint { active, edge }
    }
}

/// A pair of constraints (a, b) where edgee(a) = !edge(b)
struct ConstraintPair<W> {
    /// constraint where the edge is in its canonical form
    base: Constraint<W>,
    /// constraint corresponding to the negation of base
    negated: Constraint<W>,
}

impl<W: Time> ConstraintPair<W> {
    pub fn new_inactives(edge: Edge<W>) -> ConstraintPair<W> {
        if edge.is_canonical() {
            ConstraintPair {
                base: Constraint::new(false, edge),
                negated: Constraint::new(false, edge.negated()),
            }
        } else {
            ConstraintPair {
                base: Constraint::new(false, edge.negated()),
                negated: Constraint::new(false, edge),
            }
        }
    }
}

/// Data structures that holds all active and inactive edges in the STN.
/// Note that some edges might be represented even though they were never inserted if they are the
/// negation of an inserted edge.
struct ConstraintDB<W> {
    /// All constraints pairs, the index of this vector is the base_id of the edges in the pair.
    constraints: Vec<ConstraintPair<W>>,
    /// Maps each canonical edge to its location
    lookup: HashMap<Edge<W>, u32>,
}
impl<W: Time> ConstraintDB<W> {
    pub fn new() -> ConstraintDB<W> {
        ConstraintDB {
            constraints: vec![],
            lookup: HashMap::new(),
        }
    }

    fn find_existing(&self, edge: &Edge<W>) -> Option<EdgeID> {
        if edge.is_canonical() {
            self.lookup.get(edge).map(|&id| EdgeID::new(id, false))
        } else {
            self.lookup.get(&edge.negated()).map(|&id| EdgeID::new(id, true))
        }
    }

    /// Adds a new edge and return a pair (created, edge_id) where:
    ///  - created is false if NO new edge was inserted (it was merge with an identical edge already in the DB)
    ///  - edge_id is the id of the edge
    pub fn push_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> (bool, EdgeID) {
        let edge = Edge::new(source, target, weight);
        match self.find_existing(&edge) {
            Some(id) => {
                // edge already exists in the DB, return its id and say it wasn't created
                debug_assert_eq!(self[id].edge, edge);
                (false, id)
            }
            None => {
                // edge does not exist, record the corresponding pair and return the new id.
                let pair = ConstraintPair::new_inactives(edge);
                let base_id = self.constraints.len() as u32;
                self.lookup.insert(pair.base.edge, base_id);
                self.constraints.push(pair);
                let edge_id = EdgeID::new(base_id, edge.is_negated());
                debug_assert_eq!(self[edge_id].edge, edge);
                (true, edge_id)
            }
        }
    }

    /// Removes the last created ConstraintPair in the DB. Note that this will remove the last edge that was
    /// push THAT WAS NOT UNIFIED with an existing edge (i.e. edge_push returned : (true, _)).
    pub fn pop_last(&mut self) {
        if let Some(pair) = self.constraints.pop() {
            self.lookup.remove(&pair.base.edge);
        }
    }

    pub fn has_edge(&self, id: EdgeID) -> bool {
        id.base_id <= self.constraints.len() as u32
    }
}
impl<W> Index<EdgeID> for ConstraintDB<W> {
    type Output = Constraint<W>;

    fn index(&self, index: EdgeID) -> &Self::Output {
        let pair = &self.constraints[index.base_id as usize];
        if index.negated {
            &pair.negated
        } else {
            &pair.base
        }
    }
}
impl<W> IndexMut<EdgeID> for ConstraintDB<W> {
    fn index_mut(&mut self, index: EdgeID) -> &mut Self::Output {
        let pair = &mut self.constraints[index.base_id as usize];
        if index.negated {
            &mut pair.negated
        } else {
            &mut pair.base
        }
    }
}

type BacktrackLevel = u32;

enum Event<W> {
    Level(BacktrackLevel),
    NodeAdded,
    EdgeAdded,
    NewPendingActivation,
    EdgeActivated(EdgeID),
    ForwardUpdate {
        node: Timepoint,
        previous_dist: W,
        previous_cause: Option<EdgeID>,
    },
    BackwardUpdate {
        node: Timepoint,
        previous_dist: W,
        previous_cause: Option<EdgeID>,
    },
}

#[derive(Ord, PartialOrd, PartialEq, Eq, Debug)]
pub enum NetworkStatus<'a> {
    /// Network is fully propagated and consistent
    Consistent,
    /// Network is inconsistent, due to the presence of the given negative cycle.
    /// Note that internal edges (typically those inserted to represent lower/upper bounds) are
    /// omitted from the inconsistent set.
    Inconsistent(&'a [EdgeID]),
}

struct Distance<W> {
    forward: W,
    forward_cause: Option<EdgeID>,
    forward_pending_update: bool,
    backward: W,
    backward_cause: Option<EdgeID>,
    backward_pending_update: bool,
}

/// STN that supports:
///  - incremental edge addition and consistency checking with [Cesta96]
///  - undoing the latest changes
///  - providing explanation on inconsistency in the form of a culprit
///         set of constraints
///  - unifies new edges with previously inserted ones
///
/// Once the network reaches an inconsistent state, the only valid operation
/// is to undo the latest change go back to a consistent network. All other
/// operations have an undefined behavior.
///
/// Requirement for `W` : `W` is used internally to represent both delays
/// (weight on edges) and absolute times (bound on nodes). It is the responsibility
/// of the caller to ensure that no overflow occurs when adding an absolute and relative time,
/// either by the choice of an appropriate type (e.g. saturating add) or by the choice of
/// appropriate initial bounds.
pub struct IncSTN<W> {
    constraints: ConstraintDB<W>,
    /// Forward/Backward adjacency list containing active edges.
    active_forward_edges: Vec<Vec<EdgeID>>,
    active_backward_edges: Vec<Vec<EdgeID>>,
    distances: Vec<Distance<W>>,
    /// History of changes and made to the STN with all information necessary to undo them.
    trail: Vec<Event<W>>,
    pending_activations: VecDeque<EdgeID>,
    level: BacktrackLevel,
    /// Internal data structure to construct explanations as negative cycles.
    /// When encountering an inconsistency, this vector will be cleared and
    /// a negative cycle will be constructed in it. The explanation returned
    /// will be a slice of this vector to avoid any allocation.
    explanation: Vec<EdgeID>,
    /// Internal data structure to mark visited node during cycle extraction.
    visited: HashSet<Timepoint>, // TODO: consider using a bitset.
}

impl<W: Time> IncSTN<W> {
    /// Creates a new STN. Initially, the STN contains a single timepoint
    /// representing the origin whose domain is [0,0]. The id of this timepoint can
    /// be retrieved with the `origin()` method.
    pub fn new() -> Self {
        let mut stn = IncSTN {
            constraints: ConstraintDB::new(),
            active_forward_edges: vec![],
            active_backward_edges: vec![],
            distances: vec![],
            trail: vec![],
            pending_activations: VecDeque::new(),
            level: 0,
            explanation: vec![],
            visited: Default::default(),
        };
        let origin = stn.add_timepoint(W::zero(), W::zero());
        assert_eq!(origin, stn.origin());
        // make sure that initialization of the STN can not be undone
        stn.trail.clear();
        stn
    }
    pub fn num_nodes(&self) -> u32 {
        debug_assert_eq!(self.active_forward_edges.len(), self.active_backward_edges.len());
        self.active_forward_edges.len() as u32
    }

    pub fn origin(&self) -> Timepoint {
        Timepoint::from_index(0)
    }

    pub fn lb(&self, node: Timepoint) -> W {
        -self.distances[node].backward
    }
    pub fn ub(&self, node: Timepoint) -> W {
        self.distances[node].forward
    }

    /// Adds a new node (timepoint) to the STN with a domain of `[lb, ub]`.
    /// Returns the identifier of the newly added node.
    /// Lower and upper bounds have corresponding edges in the STN distance
    /// graph that will participate in propagation:
    ///  - `ORIGIN --(ub)--> node`
    ///  - `node --(-lb)--> ORIGIN`
    /// However, those edges are purely internal and since their IDs are not
    /// communicated, they will be omitted when appearing in the explanation of
    /// inconsistencies.  
    /// If you want for those to appear in explanation, consider setting bounds
    /// to -/+ infinity adding those edges manually.
    ///
    /// Panics if `lb > ub`. This guarantees that the network remains consistent
    /// when adding a node.
    ///
    /// It is the responsibility of the caller to ensure that the bound provided
    /// will not overflow when added to arbitrary weight of the network.
    pub fn add_timepoint(&mut self, lb: W, ub: W) -> Timepoint {
        assert!(lb <= ub);
        let id = Timepoint::from_index(self.num_nodes());
        self.active_forward_edges.push(Vec::new());
        self.active_backward_edges.push(Vec::new());
        debug_assert_eq!(id.to_index(), self.num_nodes() - 1);
        self.trail.push(NodeAdded);
        let (fwd_edge, _) = self.add_inactive_constraint(self.origin(), id, ub);
        let (bwd_edge, _) = self.add_inactive_constraint(id, self.origin(), -lb);
        // todo: these should not require propagation because they will properly set the
        //       node's domain. However mark_active will add them to the propagation queue
        self.mark_active(fwd_edge);
        self.mark_active(bwd_edge);
        self.distances.push(Distance {
            forward: ub,
            forward_cause: Some(fwd_edge),
            forward_pending_update: false,
            backward: -lb,
            backward_cause: Some(bwd_edge),
            backward_pending_update: false,
        });
        id
    }

    pub fn add_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> EdgeID {
        let id = self.add_inactive_edge(source, target, weight);
        self.mark_active(id);
        id
    }

    /// Records an INACTIVE new edge and returns its identifier.
    /// If an identical is already present in the network, then the identifier will the one of
    /// the existing edge.
    ///
    /// After calling this method, the edge is inactive and will not participate in
    /// propagation. The edge can be activated with the `mark_active()` method.
    ///
    /// Since the edge is inactive, the STN remains consistent after calling this method.
    pub fn add_inactive_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> EdgeID {
        self.add_inactive_constraint(source, target, weight).0
    }

    /// Marks an edge as active and enqueue it for propagation.
    /// No changes are committed to the network by this function until a call to `propagate_all()`
    pub fn mark_active(&mut self, edge: EdgeID) {
        debug_assert!(self.constraints.has_edge(edge));
        self.pending_activations.push_back(edge);
        self.trail.push(Event::NewPendingActivation);
    }

    /// Propagates all edges that have been marked as active since the last propagation.
    pub fn propagate_all(&mut self) -> NetworkStatus {
        while let Some(edge) = self.pending_activations.pop_front() {
            let c = &mut self.constraints[edge];
            let Edge { source, target, weight } = c.edge;
            if source == target {
                if weight < W::zero() {
                    // negative self loop: inconsistency
                    self.explanation.clear();
                    self.explanation.push(edge);
                    return NetworkStatus::Inconsistent(&self.explanation);
                } else {
                    // positive self loop : useless edge that we can ignore
                }
            } else if !c.active {
                c.active = true;
                self.active_forward_edges[source].push(edge);
                self.active_backward_edges[target].push(edge);
                self.trail.push(EdgeActivated(edge));
                // if self.propagate(edge) != NetworkStatus::Consistent;
                if let NetworkStatus::Inconsistent(explanation) = self.propagate(edge) {
                    // work around borrow checker, transmutation should be a no-op that just resets lifetimes
                    let x = unsafe { std::mem::transmute(explanation) };
                    return NetworkStatus::Inconsistent(x);
                }
            }
        }
        NetworkStatus::Consistent
    }

    /// Creates a new backtrack point that represents the STN at the point of the method call,
    /// just before the insertion of the backtrack point.
    pub fn set_backtrack_point(&mut self) -> BacktrackLevel {
        assert!(
            self.pending_activations.is_empty(),
            "Cannot set a backtrack point if a propagation is pending."
        );
        self.level += 1;
        self.trail.push(Event::Level(self.level));
        self.level
    }

    /// Revert all changes and put the STN to the state it was in immediately before the insertion of
    /// the given backtrack point.
    /// Panics if the backtrack point is not recorded.
    pub fn backtrack_to(&mut self, point: BacktrackLevel) {
        assert!(self.level >= point, "Invalid backtrack point: already backtracked upon");
        while self.level >= point {
            self.undo_to_last_backtrack_point();
        }
    }

    pub fn undo_to_last_backtrack_point(&mut self) -> Option<BacktrackLevel> {
        while let Some(ev) = self.trail.pop() {
            match ev {
                Event::Level(lvl) => {
                    self.level -= 1;
                    return Some(lvl);
                }
                NodeAdded => {
                    self.active_forward_edges.pop();
                    self.active_backward_edges.pop();
                    self.distances.pop();
                }
                EdgeAdded => {
                    self.constraints.pop_last();
                }
                NewPendingActivation => {
                    self.pending_activations.pop_back();
                }
                EdgeActivated(e) => {
                    let c = &mut self.constraints[e];
                    self.active_forward_edges[c.edge.source].pop();
                    self.active_backward_edges[c.edge.target].pop();
                    c.active = false;
                }
                Event::ForwardUpdate {
                    node,
                    previous_dist,
                    previous_cause,
                } => {
                    let x = &mut self.distances[node];
                    x.forward = previous_dist;
                    x.forward_cause = previous_cause;
                }
                Event::BackwardUpdate {
                    node,
                    previous_dist,
                    previous_cause,
                } => {
                    let x = &mut self.distances[node];
                    x.backward = previous_dist;
                    x.backward_cause = previous_cause;
                }
            }
        }
        None
    }

    /// Return a tuple `(id, created)` where id is the id of the edge and created is a boolean value that is true if the
    /// edge was created and false if it was unified with a previous instance
    fn add_inactive_constraint(&mut self, source: Timepoint, target: Timepoint, weight: W) -> (EdgeID, bool) {
        assert!(
            source.to_index() < self.num_nodes() && target.to_index() < self.num_nodes(),
            "Unrecorded node"
        );
        let (created, id) = self.constraints.push_edge(source, target, weight);
        if created {
            self.trail.push(EdgeAdded);
        }
        (id, created)
    }

    fn fdist(&self, n: Timepoint) -> W {
        self.distances[n].forward
    }
    fn bdist(&self, n: Timepoint) -> W {
        self.distances[n].backward
    }
    fn active(&self, e: EdgeID) -> bool {
        self.constraints[e].active
    }

    /// Implementation of [Cesta96]
    /// It propagates a **newly_inserted** edge in a **consistent** STN.
    fn propagate(&mut self, new_edge: EdgeID) -> NetworkStatus {
        let mut queue = VecDeque::new();
        // fast access to check if a node is in the queue
        // this can be improve with a bitset, and might not be necessary since
        // any work is guarded by the pending update flags
        let mut in_queue = HashSet::new();
        let c = &self.constraints[new_edge];
        debug_assert_ne!(
            c.edge.source, c.edge.target,
            "This algorithm does not support self loops."
        );
        let i = c.edge.source;
        let j = c.edge.target;
        queue.push_back(i);
        in_queue.insert(i);
        queue.push_back(j);
        in_queue.insert(j);
        self.distances[i].forward_pending_update = true;
        self.distances[i].backward_pending_update = true;
        self.distances[j].forward_pending_update = true;
        self.distances[j].backward_pending_update = true;

        while let Some(u) = queue.pop_front() {
            in_queue.remove(&u);
            if self.distances[u].forward_pending_update {
                for &out_edge in &self.active_forward_edges[u] {
                    // TODO(perf): we should avoid touching the constraints array by adding target and weight to forward edges
                    let c = &self.constraints[out_edge];
                    let Edge { source, target, weight } = c.edge;
                    debug_assert!(self.active(out_edge));
                    debug_assert_eq!(u, source);
                    let previous = self.fdist(target);
                    let candidate = self.fdist(source) + weight;
                    if candidate < previous {
                        if candidate < -self.bdist(target) {
                            // negative cycle
                            return NetworkStatus::Inconsistent(self.extract_cycle(new_edge));
                        }
                        self.trail.push(Event::ForwardUpdate {
                            node: target,
                            previous_dist: previous,
                            previous_cause: self.distances[target].forward_cause,
                        });
                        self.distances[target].forward = candidate;
                        self.distances[target].forward_cause = Some(out_edge);
                        self.distances[target].forward_pending_update = true;
                        if !in_queue.contains(&target) {
                            queue.push_back(target);
                            in_queue.insert(target);
                        }
                    }
                }
            }

            if self.distances[u].backward_pending_update {
                for &in_edge in &self.active_backward_edges[u] {
                    let c = &self.constraints[in_edge];
                    let Edge { source, target, weight } = c.edge;
                    debug_assert!(self.active(in_edge));
                    debug_assert_eq!(u, target);
                    let previous = self.bdist(source);
                    let candidate = self.bdist(target) + weight;
                    if candidate < previous {
                        if candidate < -self.fdist(source) {
                            // negative cycle
                            return NetworkStatus::Inconsistent(self.extract_cycle(new_edge));
                        }
                        self.trail.push(Event::BackwardUpdate {
                            node: source,
                            previous_dist: previous,
                            previous_cause: self.distances[source].backward_cause,
                        });
                        self.distances[source].backward = candidate;
                        self.distances[source].backward_cause = Some(in_edge);
                        self.distances[source].backward_pending_update = true;
                        if !in_queue.contains(&source) {
                            queue.push_back(source);
                            in_queue.insert(source);
                        }
                    }
                }
            }
            // problematic in the case of self cycles...
            self.distances[u].forward_pending_update = false;
            self.distances[u].backward_pending_update = false;
        }
        NetworkStatus::Consistent
    }

    /// Builds a cycle by following forward/backward causes until a cycle is found.
    /// Returns a set of active non-internal edges that are part of a negative cycle
    /// involving `edge`.
    /// Panics if no such cycle exists.
    fn extract_cycle(&mut self, edge: EdgeID) -> &[EdgeID] {
        let e = &self.constraints[edge];
        let Edge {
            source,
            target,
            weight: _,
        } = e.edge;

        let mut current = target;

        self.explanation.clear();
        // add the `source -> target` edge to the explanation
        self.explanation.push(edge);
        self.visited.clear();
        self.visited.insert(target);

        // follow backward causes and mark all predecessor nodes.
        while current != source && current != self.origin() {
            let next_constraint_id = self.distances[current]
                .backward_cause
                .expect("No cause on member of cycle");
            let nc = &self.constraints[next_constraint_id];
            current = nc.edge.target;
            debug_assert!(
                !self.visited.contains(&current),
                "Cycle does not involve the passed edge."
            );
            self.visited.insert(current);
        }
        let mut current = source;
        // follow forward causes until we find one visited when going up the backward causes.
        while !self.visited.contains(&current) {
            let next_constraint_id = self.distances[current]
                .forward_cause
                .expect("No cause on member of cycle");

            let nc = &self.constraints[next_constraint_id];
            self.explanation.push(next_constraint_id);
            current = nc.edge.source;
        }
        // we found a cycle of causes:  `target ----> root -----> source -> target`
        let root = current;
        debug_assert!(self.visited.contains(&root));
        // the edge `source -> target` and the path `root -----> source` is already in `self.explanation`
        // follow again the backward causes from target to add the `target ----> root` path to the explanation
        current = target;
        while current != root {
            let next_constraint_id = self.distances[current]
                .backward_cause
                .expect("No cause on member of cycle");
            let nc = &self.constraints[next_constraint_id];
            self.explanation.push(next_constraint_id);
            current = nc.edge.target;
        }
        // TODO: check that the cycle has negative length
        // debug_assert!(self.explanation.iter().map(|eid| self.constraints[*eid].edge.weight).sum()< W::zero());
        &self.explanation
    }

    // #[allow(dead_code)]
    // fn print(&self)
    // where
    //     W: Display,
    // {
    //     println!("Nodes: ");
    //     for (id, n) in self.distances.iter().enumerate() {
    //         println!(
    //             "{} [{}, {}] back_cause: {:?}  forw_cause: {:?}",
    //             id, -n.backward, n.forward, n.backward_cause, n.forward_cause
    //         );
    //     }
    //     println!("Active Edges:");
    //     for (id, &c) in self.constraints.iter().enumerate().filter(|x| x.1.active) {
    //         println!("{}: {} -- {} --> {} ", id, c.source, c.weight, c.target);
    //     }
    // }
}

impl<W: Time> Default for IncSTN<W> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "theories")]
use aries_smt::{Theory, TheoryStatus};
use std::hash::Hash;
use std::ops::Index;

#[cfg(feature = "theories")]
impl<W: Time> Theory<Edge<W>> for IncSTN<W> {
    fn record_atom(&mut self, atom: Edge<W>) -> aries_smt::AtomRecording {
        let (id, created) = self.add_inactive_constraint(atom.source, atom.target, atom.weight);
        if created {
            aries_smt::AtomRecording::newly_created(id.into())
        } else {
            aries_smt::AtomRecording::unified_with_existing(id.into())
        }
    }

    fn enable(&mut self, atom_id: aries_smt::AtomID) {
        self.mark_active(atom_id.into());
    }

    fn deduce(&mut self) -> TheoryStatus {
        match self.propagate_all() {
            NetworkStatus::Consistent => TheoryStatus::Consistent,
            NetworkStatus::Inconsistent(x) => {
                TheoryStatus::Inconsistent(x.iter().map(|e| aries_smt::AtomID::from(*e)).collect())
            }
        }
    }

    fn set_backtrack_point(&mut self) -> u32 {
        self.set_backtrack_point()
    }

    fn get_last_backtrack_point(&mut self) -> u32 {
        self.level
    }

    fn backtrack(&mut self) {
        self.undo_to_last_backtrack_point();
    }

    fn backtrack_to(&mut self, point: u32) {
        self.backtrack_to(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stn::NetworkStatus::{Consistent, Inconsistent};

    fn assert_consistent<W: Time>(stn: &mut IncSTN<W>) {
        assert_eq!(stn.propagate_all(), Consistent);
    }
    fn assert_inconsistent<W: Time>(stn: &mut IncSTN<W>, mut cycle: Vec<EdgeID>) {
        cycle.sort();
        match stn.propagate_all() {
            Consistent => panic!("Expected inconsistent network"),
            Inconsistent(exp) => {
                let mut vec: Vec<EdgeID> = exp.iter().copied().collect();
                vec.sort();
                assert_eq!(vec, cycle);
            }
        }
    }

    #[test]
    fn test_backtracking() {
        let mut stn = IncSTN::new();
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);
        assert_eq!(stn.lb(a), 0);
        assert_eq!(stn.ub(a), 10);
        assert_eq!(stn.lb(b), 0);
        assert_eq!(stn.ub(b), 10);

        stn.add_edge(stn.origin(), a, 1);
        assert_consistent(&mut stn);
        assert_eq!(stn.lb(a), 0);
        assert_eq!(stn.ub(a), 1);
        assert_eq!(stn.lb(b), 0);
        assert_eq!(stn.ub(b), 10);
        stn.set_backtrack_point();

        let ab = stn.add_edge(a, b, 5i32);
        assert_consistent(&mut stn);
        assert_eq!(stn.lb(a), 0);
        assert_eq!(stn.ub(a), 1);
        assert_eq!(stn.lb(b), 0);
        assert_eq!(stn.ub(b), 6);

        stn.set_backtrack_point();

        let ba = stn.add_edge(b, a, -6i32);
        assert_inconsistent(&mut stn, vec![ab, ba]);

        stn.undo_to_last_backtrack_point();
        assert_eq!(stn.lb(a), 0);
        assert_eq!(stn.ub(a), 1);
        assert_eq!(stn.lb(b), 0);
        assert_eq!(stn.ub(b), 6);

        stn.undo_to_last_backtrack_point();
        assert_eq!(stn.lb(a), 0);
        assert_eq!(stn.ub(a), 1);
        assert_eq!(stn.lb(b), 0);
        assert_eq!(stn.ub(b), 10);

        let x = stn.add_inactive_edge(a, b, 5i32);
        stn.mark_active(x);
        assert_eq!(stn.propagate_all(), Consistent);
        assert_eq!(stn.lb(a), 0);
        assert_eq!(stn.ub(a), 1);
        assert_eq!(stn.lb(b), 0);
        assert_eq!(stn.ub(b), 6);
    }

    #[test]
    fn test_unification() {
        // build base stn
        let mut stn = IncSTN::new();
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);

        // two identical edges should be unified
        let id1 = stn.add_edge(a, b, 1);
        let id2 = stn.add_edge(a, b, 1);
        assert_eq!(id1, id2);

        // edge negations
        let edge = Edge::new(a, b, 3); // b - a <= 3
        let not_edge = edge.negated(); // b - a > 3   <=>  a - b < -3  <=>  a - b <= -4
        assert_eq!(not_edge, Edge::new(b, a, -4));

        let id = stn.add_edge(edge.source, edge.target, edge.weight);
        let nid = stn.add_edge(not_edge.source, not_edge.target, not_edge.weight);
        assert_eq!(id.base_id(), nid.base_id());
        assert_ne!(id.is_negated(), nid.is_negated());
    }

    #[test]
    fn test_explanation() {
        let mut stn = IncSTN::new();
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);
        let c = stn.add_timepoint(0, 10);
        stn.propagate_all();

        stn.set_backtrack_point();
        let aa = stn.add_inactive_edge(a, a, -1);
        stn.mark_active(aa);
        assert_inconsistent(&mut stn, vec![aa]);

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        let ab = stn.add_edge(a, b, 2);
        let ba = stn.add_edge(b, a, -3);
        assert_inconsistent(&mut stn, vec![ab, ba]);

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        let ab = stn.add_edge(a, b, 2);
        let _ = stn.add_edge(b, a, -2);
        assert_consistent(&mut stn);
        let ba = stn.add_edge(b, a, -3);
        assert_inconsistent(&mut stn, vec![ab, ba]);

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        let ab = stn.add_edge(a, b, 2);
        let bc = stn.add_edge(b, c, 2);
        let _ = stn.add_edge(c, a, -4);
        assert_consistent(&mut stn);
        let ca = stn.add_edge(c, a, -5);
        assert_inconsistent(&mut stn, vec![ab, bc, ca]);
    }
}
