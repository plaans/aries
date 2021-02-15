#![allow(unused)] // TODO: remove
use crate::stn::Event::{EdgeActivated, EdgeAdded, NewPendingActivation};
use aries_model::assignments::Assignment;

use std::collections::{HashMap, VecDeque};
use std::ops::{IndexMut, Not};

pub type Timepoint = VarRef;
pub type W = IntCst;

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

impl std::ops::Not for EdgeID {
    type Output = Self;

    fn not(self) -> Self::Output {
        EdgeID {
            base_id: self.base_id,
            negated: !self.negated,
        }
    }
}

impl From<EdgeID> for u64 {
    fn from(e: EdgeID) -> Self {
        let base = (e.base_id << 1) as u64;
        if e.negated {
            base + 1
        } else {
            base
        }
    }
}
impl From<u64> for EdgeID {
    fn from(id: u64) -> Self {
        let base_id = (id >> 1) as u32;
        let negated = (id & 0x1) == 1;
        EdgeID::new(base_id, negated)
    }
}

/// An edge in the STN, representing the constraint `target - source <= weight`
/// An edge can be either in canonical form or in negated form.
/// Given to edges (tgt - src <= w) and (tgt -src > w) one will be in canonical form and
/// the other in negated form.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Edge {
    pub source: Timepoint,
    pub target: Timepoint,
    pub weight: W,
}

impl Edge {
    pub fn new(source: Timepoint, target: Timepoint, weight: W) -> Edge {
        Edge { source, target, weight }
    }

    fn is_negated(&self) -> bool {
        !self.is_canonical()
    }

    fn is_canonical(&self) -> bool {
        self.source < self.target || self.source == self.target && self.weight >= 0
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
            weight: -self.weight - 1,
        }
    }
}

#[derive(Clone)]
struct Constraint {
    /// True if the constraint active (participates in propagation)
    active: bool,
    edge: Edge,
    /// True if the constraint is always active.
    /// This is the case if its enabler is entails at the ground decision level
    always_active: bool,
    /// A set of enablers for this constraint.
    /// The edge becomes active once one of its enablers becomes true
    enablers: Vec<Bound>,
}
impl Constraint {
    pub fn new(active: bool, edge: Edge) -> Constraint {
        Constraint {
            active,
            edge,
            always_active: false,
            enablers: Vec::new(),
        }
    }
}

/// A pair of constraints (a, b) where edgee(a) = !edge(b)
struct ConstraintPair {
    /// constraint where the edge is in its canonical form
    base: Constraint,
    /// constraint corresponding to the negation of base
    negated: Constraint,
}

impl ConstraintPair {
    pub fn new_inactives(edge: Edge) -> ConstraintPair {
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
struct ConstraintDB {
    /// All constraints pairs, the index of this vector is the base_id of the edges in the pair.
    constraints: Vec<ConstraintPair>,
    /// Maps each canonical edge to its location
    lookup: HashMap<Edge, u32>,
    watches: Watches<EdgeID>,
}
impl ConstraintDB {
    pub fn new() -> ConstraintDB {
        ConstraintDB {
            constraints: vec![],
            lookup: HashMap::new(),
            watches: Default::default(),
        }
    }

    pub fn add_enabler(&mut self, edge: EdgeID, literal: Bound) {
        self.watches.add_watch(edge, literal);
        self[edge].enablers.push(literal);
    }

    fn find_existing(&self, edge: &Edge) -> Option<EdgeID> {
        if edge.is_canonical() {
            self.lookup.get(edge).map(|&id| EdgeID::new(id, false))
        } else {
            self.lookup.get(&edge.negated()).map(|&id| EdgeID::new(id, true))
        }
    }

    /// Adds a new edge and return a pair (created, edge_id) where:
    ///  - created is false if NO new edge was inserted (it was merge with an identical edge already in the DB)
    ///  - edge_id is the id of the edge
    ///
    /// If the edge is marked as hidden, then it will not appear in the lookup table. This will prevent
    /// it from being unified with a future edge.
    pub fn push_edge(&mut self, source: Timepoint, target: Timepoint, weight: W, hidden: bool) -> (bool, EdgeID) {
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
                if !hidden {
                    // the edge is not hidden, add it to lookup so it can be unified with existing ones
                    self.lookup.insert(pair.base.edge, base_id);
                }
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
impl Index<EdgeID> for ConstraintDB {
    type Output = Constraint;

    fn index(&self, index: EdgeID) -> &Self::Output {
        let pair = &self.constraints[index.base_id as usize];
        if index.negated {
            &pair.negated
        } else {
            &pair.base
        }
    }
}
impl IndexMut<EdgeID> for ConstraintDB {
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

enum Event {
    Level(BacktrackLevel),
    EdgeAdded,
    NewPendingActivation,
    EdgeActivated(EdgeID),
}

struct Distance {
    forward_pending_update: bool,
    backward_pending_update: bool,
}

#[derive(Default)]
struct Stats {
    num_propagations: u64,
    distance_updates: u64,
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
pub struct IncSTN {
    constraints: ConstraintDB,
    /// Forward/Backward adjacency list containing active edges.
    active_forward_edges: Vec<Vec<FwdActive>>,
    active_backward_edges: Vec<Vec<BwdActive>>,
    distances: Vec<Distance>,
    /// History of changes and made to the STN with all information necessary to undo them.
    trail: Trail<Event>,
    pending_activations: VecDeque<ActivationEvent>,
    stats: Stats,
    identity: WriterId,
    model_events: ObsTrailCursor<ModelEvent>,
    /// Internal data structure to construct explanations as negative cycles.
    /// When encountering an inconsistency, this vector will be cleared and
    /// a negative cycle will be constructed in it. The explanation returned
    /// will be a slice of this vector to avoid any allocation.
    explanation: Vec<EdgeID>,
    /// Internal data structure used by the `propagate` method to keep track of pending work.
    internal_propagate_queue: VecDeque<Timepoint>,
    internal_visited_by_cycle_extraction: RefSet<Timepoint>,
}

/// Stores the target and weight of an edge in the active forward queue.
/// This structure serves as a cache to avoid touching the `constraints` array
/// in the propagate loop.
struct FwdActive {
    target: Timepoint,
    weight: W,
    id: EdgeID,
}

/// Stores the source and weight of an edge in the active backward queue.
/// This structure serves as a cache to avoid touching the `constraints` array
/// in the propagate loop.
struct BwdActive {
    source: Timepoint,
    weight: W,
    id: EdgeID,
}

enum ActivationEvent {
    ToActivate(EdgeID),
}

impl IncSTN {
    /// Creates a new STN. Initially, the STN contains a single timepoint
    /// representing the origin whose domain is [0,0]. The id of this timepoint can
    /// be retrieved with the `origin()` method.
    pub fn new(identity: WriterId) -> Self {
        IncSTN {
            constraints: ConstraintDB::new(),
            active_forward_edges: vec![],
            active_backward_edges: vec![],
            distances: vec![],
            trail: Default::default(),
            pending_activations: VecDeque::new(),
            stats: Default::default(),
            identity,
            model_events: ObsTrailCursor::new(),
            explanation: vec![],
            internal_propagate_queue: Default::default(),
            internal_visited_by_cycle_extraction: Default::default(),
        }
    }
    pub fn num_nodes(&self) -> u32 {
        debug_assert_eq!(self.active_forward_edges.len(), self.active_backward_edges.len());
        self.active_forward_edges.len() as u32
    }

    pub fn reserve_timepoint(&mut self) {
        self.active_forward_edges.push(Vec::new());
        self.active_backward_edges.push(Vec::new());
        self.distances.push(Distance {
            forward_pending_update: false,
            backward_pending_update: false,
        });
    }

    pub fn add_reified_edge(
        &mut self,
        literal: Bound,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        model: &Model,
    ) -> EdgeID {
        let e = self
            .add_inactive_constraint(source.into(), target.into(), weight, false)
            .0;

        if model.entails(literal) {
            assert_eq!(model.discrete.entailing_level(literal), 0);
            self.constraints[e].always_active = true;
            self.mark_active(e);
        } else {
            self.constraints.add_enabler(e, literal);
            self.constraints.add_enabler(!e, !literal);
        }

        e
    }

    /// Marks an edge as active and enqueue it for propagation.
    /// No changes are committed to the network by this function until a call to `propagate_all()`
    pub fn mark_active(&mut self, edge: EdgeID) {
        debug_assert!(self.constraints.has_edge(edge));
        self.pending_activations.push_back(ActivationEvent::ToActivate(edge));
        self.trail.push(Event::NewPendingActivation);
    }

    fn build_contradiction(&self, culprits: &[EdgeID], model: &DiscreteModel) -> Contradiction {
        let mut expl = Explanation::new();
        for &edge in culprits {
            debug_assert!(self.active(edge));
            let c = &self.constraints[edge];
            if c.always_active {
                // no bound to add for this edge
                continue;
            }
            let mut literal = None;
            for &enabler in &self.constraints[edge].enablers {
                // find the first enabler that is entailed and add it it to teh explanation
                if model.entails(enabler) {
                    literal = Some(enabler);
                    break;
                }
            }
            let literal = literal.expect("No entailed enabler for this edge");
            expl.push(literal);
        }
        Contradiction::Explanation(expl)
    }

    /// Returns the enabling literal of the edge: a literal that enables the edge
    /// and is true in the provided model.
    /// Return None if the edge is always active.
    fn enabling_literal(&self, edge: EdgeID, model: &DiscreteModel) -> Option<Bound> {
        debug_assert!(self.active(edge));
        let c = &self.constraints[edge];
        if c.always_active {
            // no bound to add for this edge
            return None;
        }
        for &enabler in &c.enablers {
            // find the first enabler that is entailed and add it it to teh explanation
            if model.entails(enabler) {
                return Some(enabler);
            }
        }
        panic!("No enabling literal for this edge")
    }

    fn explain_event(
        &self,
        event: Bound,
        propagator: EdgeID,
        model: &DiscreteModel,
        out_explanation: &mut Explanation,
    ) {
        debug_assert!(self.active(propagator));
        let c = &self.constraints[propagator];
        let var = event.variable();
        let val = event.value();
        let cause = match event.relation() {
            Relation::LEQ => {
                debug_assert_eq!(var, c.edge.target);
                Bound::leq(c.edge.source, val - c.edge.weight)
            }
            Relation::GT => {
                debug_assert_eq!(var, c.edge.source);
                Bound::gt(c.edge.target, val + c.edge.weight)
            }
        };
        out_explanation.push(cause);
        if let Some(literal) = self.enabling_literal(propagator, model) {
            out_explanation.push(literal);
        }
    }

    /// Propagates all edges that have been marked as active since the last propagation.
    pub fn propagate_all(&mut self, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        while self.model_events.num_pending(model.trail()) > 0 || !self.pending_activations.is_empty() {
            // start by propagating all bounds changes before considering the new edges.
            // This necessary because cycle detection on the insertion of a new edge requires
            // a consistent STN and no interference of external bound updates.
            while let Some(ev) = self.model_events.pop(model.trail()) {
                let literal = ev.new_literal();
                for edge in self.constraints.watches.watches_on(literal) {
                    // mark active
                    debug_assert!(self.constraints.has_edge(edge));
                    self.pending_activations.push_back(ActivationEvent::ToActivate(edge));
                    self.trail.push(Event::NewPendingActivation);
                }
                if matches!(ev.cause, Cause::Inference(x) if x.writer == self.identity) {
                    // we generated this event ourselves, we can safely ignore it as it would have been handled
                    // immediately
                    continue;
                }
                self.propagate_bound_change(literal, model)?;
            }
            while let Some(event) = self.pending_activations.pop_front() {
                let ActivationEvent::ToActivate(edge) = event;
                let c = &mut self.constraints[edge];
                if !c.active {
                    c.active = true;
                    let Edge { source, target, weight } = c.edge;
                    if source == target {
                        // we are in a self loop, that must must handled separately since they are trivial
                        // to handle and not supported by the propagation loop
                        if weight < 0 {
                            // negative self loop: inconsistency
                            self.explanation.clear();
                            self.explanation.push(edge);
                            return Err(self.build_contradiction(&self.explanation, model));
                        } else {
                            // positive self loop : useless edge that we can ignore
                        }
                    } else {
                        self.active_forward_edges[source].push(FwdActive {
                            target,
                            weight,
                            id: edge,
                        });
                        self.active_backward_edges[target].push(BwdActive {
                            source,
                            weight,
                            id: edge,
                        });
                        self.trail.push(EdgeActivated(edge));
                        self.propagate_new_edge(edge, model)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Creates a new backtrack point that represents the STN at the point of the method call,
    /// just before the insertion of the backtrack point.
    pub fn set_backtrack_point(&mut self) -> BacktrackLevel {
        assert!(
            self.pending_activations.is_empty(),
            "Cannot set a backtrack point if a propagation is pending. \
            The code introduced in this commit should enable this but has not been thoroughly tested yet."
        );
        self.trail.save_state()
    }

    pub fn undo_to_last_backtrack_point(&mut self) -> Option<BacktrackLevel> {
        // remove pending activations
        // invariant: there are no pending activation when saving the state
        self.pending_activations.clear();

        // undo changes since the last backtrack point
        let constraints = &mut self.constraints;
        let pending_activations = &mut self.pending_activations;
        let active_forward_edges = &mut self.active_forward_edges;
        let active_backward_edges = &mut self.active_backward_edges;
        self.trail.restore_last_with(|ev| match ev {
            Event::Level(_) => panic!(),
            EdgeAdded => constraints.pop_last(),
            NewPendingActivation => {
                pending_activations.pop_back();
            }
            EdgeActivated(e) => {
                let c = &mut constraints[e];
                active_forward_edges[c.edge.source].pop();
                active_backward_edges[c.edge.target].pop();
                c.active = false;
            }
        });

        None
    }

    /// Return a tuple `(id, created)` where id is the id of the edge and created is a boolean value that is true if the
    /// edge was created and false if it was unified with a previous instance
    fn add_inactive_constraint(
        &mut self,
        source: Timepoint,
        target: Timepoint,
        weight: W,
        hidden: bool,
    ) -> (EdgeID, bool) {
        while u32::from(source) >= self.num_nodes() || u32::from(target) >= self.num_nodes() {
            self.reserve_timepoint();
        }
        let (created, id) = self.constraints.push_edge(source, target, weight, hidden);
        if created {
            self.trail.push(EdgeAdded);
        }
        (id, created)
    }

    fn active(&self, e: EdgeID) -> bool {
        self.constraints[e].active
    }

    fn has_edges(&self, var: Timepoint) -> bool {
        usize::from(var) < self.distances.len()
    }

    fn propagate_bound_change(&mut self, bound: Bound, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        if !self.has_edges(bound.variable()) {
            return Ok(());
        }
        // TODO: we should make sure that those are clear
        for x in &mut self.distances {
            x.forward_pending_update = false;
            x.backward_pending_update = false;
        }
        self.internal_propagate_queue.clear(); // reset to make sure that we are not in a dirty state
        let var = bound.variable();
        match bound.relation() {
            Relation::LEQ => {
                self.internal_propagate_queue.push_back(var);
                self.distances[var].forward_pending_update = true;
            }
            Relation::GT => {
                self.internal_propagate_queue.push_back(var);
                self.distances[var].backward_pending_update = true;
            }
        }

        self.run_propagation_loop(model, None, None)
    }

    /// Implementation of [Cesta96]
    /// It propagates a **newly_inserted** edge in a **consistent** STN.
    fn propagate_new_edge(&mut self, new_edge: EdgeID, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        self.internal_propagate_queue.clear(); // reset to make sure we are not in a dirty state
        let c = &self.constraints[new_edge];
        debug_assert_ne!(
            c.edge.source, c.edge.target,
            "This algorithm does not support self loops."
        );
        let new_edge_source = c.edge.source;
        let new_edge_target = c.edge.target;
        self.internal_propagate_queue.push_back(new_edge_source);
        self.internal_propagate_queue.push_back(new_edge_target);

        // TODO: we should make sure that those are clear
        for x in &mut self.distances {
            x.forward_pending_update = false;
            x.backward_pending_update = false;
        }
        self.distances[new_edge_source].forward_pending_update = true;
        self.distances[new_edge_source].backward_pending_update = true;
        self.distances[new_edge_target].forward_pending_update = true;
        self.distances[new_edge_target].backward_pending_update = true;
        let mut target_updated_ub = false;
        let mut source_updated_lb = false;

        self.run_propagation_loop(model, Some(new_edge_source), Some(new_edge_target))
    }

    fn run_propagation_loop(
        &mut self,
        model: &mut DiscreteModel,
        new_edge_source: Option<Timepoint>,
        new_edge_target: Option<Timepoint>,
    ) -> Result<(), Contradiction> {
        self.stats.num_propagations += 1;

        let mut source_updated_lb = false;
        let mut target_updated_ub = false;
        while let Some(u) = self.internal_propagate_queue.pop_front() {
            if self.distances[u].forward_pending_update {
                for &FwdActive {
                    target,
                    weight,
                    id: out_edge,
                } in &self.active_forward_edges[u]
                {
                    let source = u;

                    debug_assert_eq!(&Edge { source, target, weight }, &self.constraints[out_edge].edge);
                    debug_assert!(self.active(out_edge));

                    let previous = model.ub(target);
                    let candidate = model.ub(source).saturating_add(weight);
                    if candidate < previous {
                        model.set_ub(target, candidate, self.identity.cause(out_edge))?;
                        self.distances[target].forward_pending_update = true;
                        self.stats.distance_updates += 1;

                        if new_edge_target == Some(target) {
                            if target_updated_ub {
                                // updated twice, there is a cycle. See cycle detection in [Cesta96]
                                return Err(self.extract_ub_cycle(target, model).into());
                            } else {
                                target_updated_ub = true;
                            }
                        }

                        // note: might result in having this more than once in the queue.
                        // this is ok though since any further work is guarded by the forward/backward pending updates
                        self.internal_propagate_queue.push_back(target);
                    }
                }
            }

            if self.distances[u].backward_pending_update {
                for &BwdActive {
                    source,
                    weight,
                    id: in_edge,
                } in &self.active_backward_edges[u]
                {
                    let target = u;

                    debug_assert_eq!(&Edge { source, target, weight }, &self.constraints[in_edge].edge);
                    debug_assert!(self.active(in_edge));

                    let previous = -model.lb(source);
                    let candidate = (-model.lb(target)).saturating_add(weight);
                    if candidate < previous {
                        model.set_lb(source, -candidate, self.identity.cause(in_edge))?;
                        self.distances[source].backward_pending_update = true;
                        self.stats.distance_updates += 1;

                        if new_edge_source == Some(source) {
                            if source_updated_lb {
                                // updated twice, there is a cycle. See cycle detection in [Cesta96]
                                return Err(self.extract_lb_cycle(source, model).into());
                            } else {
                                source_updated_lb = true;
                            }
                        }

                        self.internal_propagate_queue.push_back(source); // idem, might result in more than once in the queue
                    }
                }
            }
            // problematic in the case of self cycles...
            self.distances[u].forward_pending_update = false;
            self.distances[u].backward_pending_update = false;
        }
        Ok(())
    }

    fn extract_ub_cycle(&self, tp: Timepoint, model: &DiscreteModel) -> Explanation {
        let mut expl = Explanation::new();
        let mut curr = tp;
        let mut cycle_length = 0;
        loop {
            let lit = Bound::leq(curr, model.ub(curr));
            debug_assert!(model.entails(lit));
            let ev = model.implying_event(lit).unwrap();
            debug_assert_eq!(ev.decision_level as u32, self.trail.num_saved());
            let ev = model.get_event(ev);
            let edge = match ev.cause {
                Cause::Decision => panic!(),
                Cause::Inference(cause) => EdgeID::from(cause.payload),
            };
            let c = &self.constraints[edge];
            debug_assert_eq!(c.edge.target, curr);
            cycle_length += c.edge.weight;
            curr = c.edge.source;
            if let Some(trigger) = self.enabling_literal(edge, model) {
                expl.push(trigger);
            }
            if curr == tp {
                debug_assert!(cycle_length < 0);
                break expl;
            }
        }
    }

    fn extract_lb_cycle(&self, tp: Timepoint, model: &DiscreteModel) -> Explanation {
        let mut expl = Explanation::new();
        let mut curr = tp;
        let mut cycle_length = 0;
        loop {
            let lit = Bound::geq(curr, model.lb(curr));
            let ev = model.implying_event(lit).unwrap();
            debug_assert_eq!(ev.decision_level as u32, self.trail.num_saved());
            let ev = model.get_event(ev);
            let edge = match ev.cause {
                Cause::Decision => panic!(),
                Cause::Inference(cause) => EdgeID::from(cause.payload),
            };
            let c = &self.constraints[edge];
            debug_assert_eq!(c.edge.source, curr);
            cycle_length += c.edge.weight;
            curr = c.edge.target;
            if let Some(trigger) = self.enabling_literal(edge, model) {
                expl.push(trigger);
            }
            if curr == tp {
                debug_assert!(cycle_length < 0);
                break expl;
            }
        }
    }

    // /// Extracts a cycle from `culprit` following backward causes first.
    // /// The method will write the cycle to `self.explanation`.
    // /// If there is no cycle visible from the causes reachable from `culprit`,
    // /// the method might loop indefinitely or panic.
    // fn extract_cycle_impl(&mut self, culprit: Timepoint) {
    //     let mut current = culprit;
    //     self.explanation.clear();
    //     let origin = self.origin();
    //     let visited = &mut self.internal_visited_by_cycle_extraction;
    //     visited.clear();
    //     // follow backward causes until finding a cycle, or reaching the origin
    //     // all visited edges are added to the explanation.
    //     loop {
    //         visited.insert(current);
    //         let next_constraint_id = self.distances[current]
    //             .forward_cause
    //             .expect("No cause on member of cycle");
    //         let next = self.constraints[next_constraint_id].edge.source;
    //         self.explanation.push(next_constraint_id);
    //         if next == current {
    //             // the edge is self loop which is only allowed on the origin. This mean that we have reached the origin.
    //             // we don't want to add this edge to the cycle, so we exit early.
    //             debug_assert!(current == origin, "Self loop only present on origin");
    //             break;
    //         }
    //         current = next;
    //         if current == culprit {
    //             // we have found the cycle. Return immediately, the cycle is written to self.explanation
    //             return;
    //         } else if current == origin {
    //             // reached the origin, nothing more we can do by following backward causes.
    //             break;
    //         } else if visited.contains(current) {
    //             // cycle, that does not goes through culprit
    //             let cycle_start = self
    //                 .explanation
    //                 .iter()
    //                 .position(|x| current == self.constraints[*x].edge.target)
    //                 .unwrap();
    //             self.explanation.drain(0..cycle_start).count();
    //             return;
    //         }
    //     }
    //     // remember how many edges were added to the explanation in case we need to remove them
    //     let added_by_backward_pass = self.explanation.len();
    //
    //     // complete the cycle by following forward causes
    //     let mut current = culprit;
    //     visited.clear();
    //     loop {
    //         visited.insert(current);
    //         let next_constraint_id = self.distances[current]
    //             .backward_cause
    //             .expect("No cause on member of cycle");
    //
    //         self.explanation.push(next_constraint_id);
    //         current = self.constraints[next_constraint_id].edge.target;
    //
    //         if current == origin {
    //             // we have completed the previous cycle involving the origin.
    //             // return immediately, the cycle is already written to self.explanation
    //             return;
    //         } else if current == culprit {
    //             // we have reached a cycle through forward edges only.
    //             // before returning, we need to remove the edges that were added by the backward causes.
    //             self.explanation.drain(0..added_by_backward_pass).count();
    //             return;
    //         } else if visited.contains(current) {
    //             // cycle, that does not goes through culprit
    //             // find the start of the cycle. we start looking in the edges added by the current pass.
    //             let cycle_start = self.explanation[added_by_backward_pass..]
    //                 .iter()
    //                 .position(|x| current == self.constraints[*x].edge.source)
    //                 .unwrap();
    //             // prefix to remove consists of the edges added in the previous pass + the ones
    //             // added by this pass before the beginning of the cycle
    //             let cycle_start = added_by_backward_pass + cycle_start;
    //             // remove the prefix from the explanation
    //             self.explanation.drain(0..cycle_start).count();
    //             return;
    //         }
    //     }
    // }

    // /// Builds a negative cycle involving `culprit` by following forward/backward causes until a cycle is found.
    // /// If no such cycle exists, the method might panic or loop indefinitely
    // fn extract_cycle(&mut self, culprit: Timepoint) -> &[EdgeID] {
    //     self.extract_cycle_impl(culprit);
    //
    //     let cycle = &self.explanation;
    //
    //     debug_assert!(
    //         cycle
    //             .iter()
    //             .fold(0, |acc, eid| acc + self.constraints[*eid].edge.weight)
    //             < 0,
    //         "Cycle extraction returned a cycle with a non-negative length."
    //     );
    //     cycle
    // }

    pub fn print_stats(&self) {
        println!("# nodes: {}", self.num_nodes());
        println!("# constraints: {}", self.constraints.constraints.len());
        println!("# propagations: {}", self.stats.num_propagations);
        println!("# domain updates: {}", self.stats.distance_updates);
    }
}

use aries_backtrack::{ObsTrail, ObsTrailCursor, Trail};
use aries_model::lang::{Fun, IAtom, IVar, IntCst, VarRef};
use aries_solver::solver::{Binding, BindingResult};

use aries_solver::{Contradiction, Theory};
use std::hash::Hash;
use std::ops::Index;

type ModelEvent = aries_model::int_model::domains::Event;

use aries_backtrack::Backtrack;
use aries_collections::set::RefSet;
use aries_model::bounds::{Bound, Relation, Watches};
use aries_model::expressions::ExprHandle;
use aries_model::int_model::{Cause, DiscreteModel, EmptyDomain, Explanation};
use aries_model::{Model, WModel, WriterId};
use std::collections::hash_map::Entry;
use std::convert::*;
use std::num::NonZeroU32;

impl Theory for IncSTN {
    fn identity(&self) -> WriterId {
        self.identity
    }

    fn bind(
        &mut self,
        literal: Bound,
        expr: ExprHandle,
        model: &mut Model,
        queue: &mut ObsTrail<Binding>,
    ) -> BindingResult {
        let expr = model.expressions.get(expr);
        match expr.fun {
            Fun::Leq => {
                let a = IAtom::try_from(expr.args[0]).expect("type error");
                let b = IAtom::try_from(expr.args[1]).expect("type error");
                let va = match a.var {
                    Some(v) => v,
                    None => panic!("leq with no variable on the left side"),
                };
                let vb = match b.var {
                    Some(v) => v,
                    None => panic!("leq with no variable on the right side"),
                };

                // va + da <= vb + db    <=>   va - vb <= db - da
                self.add_reified_edge(literal, vb, va, b.shift - a.shift, model);

                BindingResult::Enforced
            }
            Fun::Eq => {
                let a = IAtom::try_from(expr.args[0]).expect("type error");
                let b = IAtom::try_from(expr.args[1]).expect("type error");
                let x = model.leq(a, b);
                let y = model.leq(b, a);
                queue.push(Binding::new(literal, model.and2(x, y)));
                BindingResult::Refined
            }

            _ => BindingResult::Unsupported,
        }
    }

    fn propagate(&mut self, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        self.propagate_all(model)
    }

    fn explain(&mut self, event: Bound, context: u64, model: &DiscreteModel, out_explanation: &mut Explanation) {
        let edge_id = EdgeID::from(context);
        self.explain_event(event, edge_id, model, out_explanation);
    }

    fn print_stats(&self) {
        self.print_stats()
    }
}

impl Backtrack for IncSTN {
    fn save_state(&mut self) -> u32 {
        self.set_backtrack_point()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.undo_to_last_backtrack_point();
    }
}

struct STN {
    stn: IncSTN,
    model: Model,
    tautology: Bound,
}
impl STN {
    pub fn new() -> Self {
        let mut model = Model::new();
        let true_var = model.new_ivar(1, 1, "True");
        let tautology = Bound::geq(true_var, 1);
        let stn = IncSTN::new(model.new_write_token());
        STN { stn, model, tautology }
    }

    pub fn add_timepoint(&mut self, lb: W, ub: W) -> Timepoint {
        self.model.new_ivar(lb, ub, "").into()
    }

    pub fn set_lb(&mut self, timepoint: Timepoint, lb: W) {
        self.model.discrete.set_lb(timepoint, lb, Cause::Decision).unwrap();
    }

    pub fn set_ub(&mut self, timepoint: Timepoint, ub: W) {
        self.model.discrete.set_ub(timepoint, ub, Cause::Decision).unwrap();
    }

    pub fn add_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> EdgeID {
        self.stn
            .add_reified_edge(self.tautology, source, target, weight, &self.model)
    }

    pub fn add_reified_edge(&mut self, literal: Bound, source: Timepoint, target: Timepoint, weight: W) -> EdgeID {
        self.stn.add_reified_edge(literal, source, target, weight, &self.model)
    }

    pub fn add_inactive_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> Bound {
        let v = self
            .model
            .new_bvar(format!("reif({:?} -- {} --> {:?})", source, weight, target));
        let activation = v.true_lit();
        self.add_reified_edge(activation, source, target, weight);
        activation
    }

    pub fn mark_active(&mut self, edge: Bound) {
        self.model.discrete.decide(edge).unwrap();
    }

    pub fn propagate_all(&mut self) -> Result<(), Contradiction> {
        self.stn.propagate_all(&mut self.model.discrete)
    }

    pub fn set_backtrack_point(&mut self) {
        self.model.save_state();
        self.stn.set_backtrack_point();
    }

    pub fn undo_to_last_backtrack_point(&mut self) {
        self.model.restore_last();
        self.stn.undo_to_last_backtrack_point();
    }

    fn assert_consistent(&mut self) {
        assert!(self.propagate_all().is_ok());
    }

    fn assert_inconsistent<X>(&mut self, mut _err: Vec<X>) {
        assert!(self.propagate_all().is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aries_model::WriterId;

    #[test]
    fn test_edge_id_conversions() {
        fn check_rountrip(i: u64) {
            let edge_id = EdgeID::from(i);
            let i_new = u64::from(edge_id);
            assert_eq!(i, i_new);
            let edge_id_new = EdgeID::from(i_new);
            assert_eq!(edge_id, edge_id_new);
        }

        // check_rountrip(0);
        check_rountrip(1);
        check_rountrip(2);
        check_rountrip(3);
        check_rountrip(4);

        fn check_rountrip2(edge_id: EdgeID) {
            let i = u64::from(edge_id);
            let edge_id_new = EdgeID::from(i);
            assert_eq!(edge_id, edge_id_new);
        }
        check_rountrip2(EdgeID::new(0, true));
        check_rountrip2(EdgeID::new(0, false));
        check_rountrip2(EdgeID::new(1, true));
        check_rountrip2(EdgeID::new(1, false));
    }

    #[test]
    fn test_propagation() {
        let s = &mut STN::new();
        let a = s.add_timepoint(0, 10);
        let b = s.add_timepoint(0, 10);

        let assert_bounds = |stn: &STN, a_lb, a_ub, b_lb, b_ub| {
            assert_eq!(stn.model.bounds(IVar::new(a)), (a_lb, a_ub));
            assert_eq!(stn.model.bounds(IVar::new(b)), (b_lb, b_ub));
        };

        assert_bounds(s, 0, 10, 0, 10);
        s.set_ub(a, 3);
        s.add_edge(a, b, 5);
        s.assert_consistent();

        assert_bounds(s, 0, 3, 0, 8);

        s.set_ub(a, 1);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 6);

        let x = s.add_inactive_edge(a, b, 3);
        s.mark_active(x);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 4);
    }

    #[test]
    fn test_backtracking() {
        let s = &mut STN::new();
        let a = s.add_timepoint(0, 10);
        let b = s.add_timepoint(0, 10);

        let assert_bounds = |stn: &STN, a_lb, a_ub, b_lb, b_ub| {
            assert_eq!(stn.model.bounds(IVar::new(a)), (a_lb, a_ub));
            assert_eq!(stn.model.bounds(IVar::new(b)), (b_lb, b_ub));
        };

        assert_bounds(s, 0, 10, 0, 10);

        s.set_ub(a, 1);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 10);
        s.set_backtrack_point();

        let ab = s.add_edge(a, b, 5i32);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 6);

        s.set_backtrack_point();

        let ba = s.add_edge(b, a, -6i32);
        s.assert_inconsistent(vec![ab, ba]);

        s.undo_to_last_backtrack_point();
        assert_bounds(s, 0, 1, 0, 6);

        s.undo_to_last_backtrack_point();
        assert_bounds(s, 0, 1, 0, 10);

        let x = s.add_inactive_edge(a, b, 5i32);
        s.mark_active(x);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 6);
    }

    #[test]
    fn test_unification() {
        // build base stn
        let mut stn = STN::new();
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
        let mut stn = &mut STN::new();
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);
        let c = stn.add_timepoint(0, 10);
        stn.propagate_all();

        stn.set_backtrack_point();
        let aa = stn.add_inactive_edge(a, a, -1);
        stn.mark_active(aa);
        stn.assert_inconsistent(vec![aa]);

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        let ab = stn.add_edge(a, b, 2);
        let ba = stn.add_edge(b, a, -3);
        stn.assert_inconsistent(vec![ab, ba]);

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        let ab = stn.add_edge(a, b, 2);
        let _ = stn.add_edge(b, a, -2);
        stn.assert_consistent();
        let ba = stn.add_edge(b, a, -3);
        stn.assert_inconsistent(vec![ab, ba]);

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        let ab = stn.add_edge(a, b, 2);
        let bc = stn.add_edge(b, c, 2);
        let _ = stn.add_edge(c, a, -4);
        stn.assert_consistent();
        let ca = stn.add_edge(c, a, -5);
        stn.assert_inconsistent(vec![ab, bc, ca]);
    }
}
