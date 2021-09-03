use crate::distances::DijkstraState;
use crate::theory::Event::{EdgeActivated, EdgeAdded};
use aries_backtrack::Backtrack;
use aries_backtrack::{DecLvl, ObsTrail, ObsTrailCursor, Trail};
use aries_collections::ref_store::{RefMap, RefVec};
use aries_collections::set::RefSet;
use aries_model::assignments::Assignment;
use aries_model::bounds::{BoundValue, BoundValueAdd, Lit, VarBound, Watches};
use aries_model::expressions::ExprHandle;
use aries_model::lang::{Fun, IAtom, IntCst, VarRef};
use aries_model::state::OptDomains;
use aries_model::state::*;
use aries_model::{Model, WriterId};
use aries_solver::solver::{Binding, BindingResult};
use aries_solver::{Contradiction, Theory};
use env_param::EnvParam;
use std::collections::{HashMap, VecDeque};
use std::convert::*;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Index;
use std::ops::IndexMut;
use std::str::FromStr;

type ModelEvent = aries_model::state::Event;

pub type Timepoint = VarRef;
pub type W = IntCst;

pub static STN_THEORY_PROPAGATION: EnvParam<TheoryPropagationLevel> =
    EnvParam::new("ARIES_STN_THEORY_PROPAGATION", "bounds");
pub static STN_DEEP_EXPLANATION: EnvParam<bool> = EnvParam::new("ARIES_STN_DEEP_EXPLANATION", "false");
pub static STN_EXTENSIVE_TESTS: EnvParam<bool> = EnvParam::new("ARIES_STN_EXTENSIVE_TESTS", "false");

/// Describes which part of theory propagation should be enabled.
#[derive(Copy, Clone, Debug)]
pub enum TheoryPropagationLevel {
    /// No theory propagation.
    None,
    /// Theory propagation should only be performed on bound updates.
    /// This is typically quite efficient since no shortest path must be recomputed.
    Bounds,
    /// Theory propagation should only be performed on new edge additions.
    /// This can very costly as on should compute shortest paths in the STN graph.
    Edges,
    /// Enable theory propagation both on edge addition and bound update.
    Full,
}
impl TheoryPropagationLevel {
    pub fn bounds(&self) -> bool {
        match self {
            TheoryPropagationLevel::None | TheoryPropagationLevel::Edges => false,
            TheoryPropagationLevel::Bounds | TheoryPropagationLevel::Full => true,
        }
    }

    pub fn edges(&self) -> bool {
        match self {
            TheoryPropagationLevel::None | TheoryPropagationLevel::Bounds => false,
            TheoryPropagationLevel::Edges | TheoryPropagationLevel::Full => true,
        }
    }
}

impl FromStr for TheoryPropagationLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(TheoryPropagationLevel::None),
            "bounds" => Ok(TheoryPropagationLevel::Bounds),
            "edges" => Ok(TheoryPropagationLevel::Edges),
            "full" => Ok(TheoryPropagationLevel::Full),
            x => Err(format!(
                "Unknown theory propagation level: {}. Valid options: none, bounds, edges, full",
                x
            )),
        }
    }
}

/// Collect all options to be used by the `StnInc` module.
///
/// The default value of all parameters can be set through environment variables.
#[derive(Clone, Debug)]
pub struct StnConfig {
    /// If true, then the Stn will do extended propagation to infer which inactive
    /// edges cannot become active without creating a negative cycle.
    pub theory_propagation: TheoryPropagationLevel,
    /// If true, the explainer will do its best to build explanations that only contain the enabling literal
    /// of constraints by recursively looking at the propagation chain that caused the literal to be set
    /// and adding the enabler of each constraint along this path.
    pub deep_explanation: bool,
    /// If true, extensive and very expensive tests will be made in debug mode.
    pub extensive_tests: bool,
}

impl Default for StnConfig {
    fn default() -> Self {
        StnConfig {
            theory_propagation: STN_THEORY_PROPAGATION.get(),
            deep_explanation: STN_DEEP_EXPLANATION.get(),
            extensive_tests: STN_EXTENSIVE_TESTS.get(),
        }
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
pub struct EdgeId(u32);
impl EdgeId {
    #[inline]
    fn new(base_id: u32, negated: bool) -> EdgeId {
        if negated {
            EdgeId((base_id << 1) + 1)
        } else {
            EdgeId(base_id << 1)
        }
    }

    #[inline]
    pub fn base_id(&self) -> u32 {
        self.0 >> 1
    }

    #[inline]
    pub fn is_negated(&self) -> bool {
        self.0 & 0x1 == 1
    }

    /// Id of the forward (from source to target) view of this edge
    fn forward(self) -> DirEdge {
        DirEdge::forward(self)
    }

    /// Id of the backward view (from target to source) of this edge
    fn backward(self) -> DirEdge {
        DirEdge::backward(self)
    }
}

impl std::ops::Not for EdgeId {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        EdgeId(self.0 ^ 0x1)
    }
}

impl From<EdgeId> for u32 {
    fn from(e: EdgeId) -> Self {
        e.0
    }
}
impl From<u32> for EdgeId {
    fn from(id: u32) -> Self {
        EdgeId(id)
    }
}

impl From<EdgeId> for usize {
    fn from(e: EdgeId) -> Self {
        e.0 as usize
    }
}
impl From<usize> for EdgeId {
    fn from(id: usize) -> Self {
        EdgeId(id as u32)
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

/// A directional constraint representing the fact that an update on the `source` bound
/// should be reflected on the `target` bound.
///
/// From a classical STN edge `source -- weight --> target` there will be two directional constraints:
///   - ub(source) = X   implies   ub(target) <= X + weight
///   - lb(target) = X   implies   lb(source) >= X - weight
#[derive(Clone, Debug)]
struct DirConstraint {
    source: VarBound,
    target: VarBound,
    weight: BoundValueAdd,
    /// Non-empty if the constraint active (participates in propagation)
    /// If the enabler is Lit::TRUE, then the constraint can be assumed to be always active
    enabler: Option<Lit>,
    /// A set of potential enablers for this constraint.
    /// The edge becomes active once one of its enablers becomes true
    enablers: Vec<Lit>,
}
impl DirConstraint {
    /// source <= X   =>   target <= X + weight
    pub fn forward(edge: Edge) -> DirConstraint {
        DirConstraint {
            source: VarBound::ub(edge.source),
            target: VarBound::ub(edge.target),
            weight: BoundValueAdd::on_ub(edge.weight),
            enabler: None,
            enablers: vec![],
        }
    }

    /// target >= X   =>   source >= X - weight
    pub fn backward(edge: Edge) -> DirConstraint {
        DirConstraint {
            source: VarBound::lb(edge.target),
            target: VarBound::lb(edge.source),
            weight: BoundValueAdd::on_lb(-edge.weight),
            enabler: None,
            enablers: vec![],
        }
    }

    pub fn as_edge(&self) -> Edge {
        if self.source.is_ub() {
            debug_assert!(self.target.is_ub());
            Edge {
                source: self.source.variable(),
                target: self.target.variable(),
                weight: self.weight.as_ub_add(),
            }
        } else {
            debug_assert!(self.target.is_lb());
            Edge {
                source: self.target.variable(),
                target: self.source.variable(),
                weight: -self.weight.as_lb_add(),
            }
        }
    }
}

/// A pair of constraints (a, b) where edge(a) = !edge(b)
struct ConstraintPair {
    /// constraint where the edge is in its canonical form
    base_forward: DirConstraint,
    base_backward: DirConstraint,
    /// constraint corresponding to the negation of base
    negated_forward: DirConstraint,
    negated_backward: DirConstraint,
}

impl ConstraintPair {
    pub fn new_inactives(edge: Edge) -> ConstraintPair {
        let edge = if edge.is_canonical() { edge } else { edge.negated() };
        ConstraintPair {
            base_forward: DirConstraint::forward(edge),
            base_backward: DirConstraint::backward(edge),
            negated_forward: DirConstraint::forward(edge.negated()),
            negated_backward: DirConstraint::backward(edge.negated()),
        }
    }
}

/// Represents an edge together with a particular propagation direction:
///  - forward (source to target)
///  - backward (target to source)
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub(crate) struct DirEdge(u32);

impl DirEdge {
    /// Forward view of the given edge
    pub fn forward(e: EdgeId) -> Self {
        DirEdge(u32::from(e) << 1)
    }

    /// Backward view of the given edge
    pub fn backward(e: EdgeId) -> Self {
        DirEdge((u32::from(e) << 1) + 1)
    }

    #[allow(unused)]
    pub fn is_forward(self) -> bool {
        (u32::from(self) & 0x1) == 0
    }

    /// The edge underlying this projection
    pub fn edge(self) -> EdgeId {
        EdgeId::from(self.0 >> 1)
    }
}
impl From<DirEdge> for usize {
    fn from(e: DirEdge) -> Self {
        e.0 as usize
    }
}
impl From<usize> for DirEdge {
    fn from(u: usize) -> Self {
        DirEdge(u as u32)
    }
}
impl From<DirEdge> for u32 {
    fn from(e: DirEdge) -> Self {
        e.0
    }
}
impl From<u32> for DirEdge {
    fn from(u: u32) -> Self {
        DirEdge(u)
    }
}

/// Data structures that holds all active and inactive edges in the STN.
/// Note that some edges might be represented even though they were never inserted if they are the
/// negation of an inserted edge.
#[derive(Clone)]
struct ConstraintDb {
    /// All directional constraints.
    ///
    /// Each time a new edge is create for `DirConstraint` will be added
    /// - forward view of the canonical edge
    /// - backward view of the canonical edge
    /// - forward view of the negated edge
    /// - backward view of the negated edge
    constraints: RefVec<DirEdge, DirConstraint>,
    /// Maps each canonical edge to its base ID.
    lookup: HashMap<Edge, u32>,
    /// Associates literals to the edges that should be activated when they become true
    watches: Watches<DirEdge>,
    edges: RefVec<VarBound, Vec<EdgeTarget>>,
    /// Index of the next constraint that has not been returned yet by the `next_new_constraint` method.
    next_new_constraint: usize,
}

#[derive(Copy, Clone, Debug)]
struct EdgeTarget {
    target: VarBound,
    weight: BoundValueAdd,
    /// Literal that is true if and only if the edge must be present in the network.
    /// Note that handling of optional variables might allow and edge to propagate even it is not known
    /// to be present yet.
    presence: Lit,
}

impl ConstraintDb {
    pub fn new() -> ConstraintDb {
        ConstraintDb {
            constraints: Default::default(),
            lookup: HashMap::new(),
            watches: Default::default(),
            edges: Default::default(),
            next_new_constraint: 0,
        }
    }

    /// A function that acts as a one time iterator over constraints.
    /// It can be used to check if new constraints have been added since last time this method was called.
    pub fn next_new_constraint(&mut self) -> Option<&DirConstraint> {
        if self.next_new_constraint < self.constraints.len() {
            let out = &self.constraints[self.next_new_constraint.into()];
            self.next_new_constraint += 1;
            Some(out)
        } else {
            None
        }
    }

    /// Record the fact that, when `literal` becomes true, the given edge
    /// should be made active in both directions.
    pub fn add_enabler(&mut self, edge: EdgeId, literal: Lit) {
        self.add_directed_enabler(edge.forward(), literal, Some(literal));
        self.add_directed_enabler(edge.backward(), literal, Some(literal));
    }

    /// Record the fact that:
    ///  - if `propagation_enabler` is true, then propagation of the directed edge should be made active
    ///  - if the edge is inconsistent with the rest of the network, then the presence literal should be false.
    pub fn add_directed_enabler(&mut self, edge: DirEdge, propagation_enabler: Lit, presence_literal: Option<Lit>) {
        self.watches.add_watch(edge, propagation_enabler);
        let constraint = &mut self.constraints[edge];
        constraint.enablers.push(propagation_enabler);
        self.edges.fill_with(constraint.source, Vec::new);
        if let Some(presence_literal) = presence_literal {
            self.edges[constraint.source].push(EdgeTarget {
                target: constraint.target,
                weight: constraint.weight,
                presence: presence_literal,
            });
        }
    }

    pub fn potential_out_edges(&self, source: VarBound) -> &[EdgeTarget] {
        if self.edges.contains(source) {
            &self.edges[source]
        } else {
            &[]
        }
    }

    fn find_existing(&self, edge: &Edge) -> Option<EdgeId> {
        if edge.is_canonical() {
            self.lookup.get(edge).map(|&id| EdgeId::new(id, false))
        } else {
            self.lookup.get(&edge.negated()).map(|&id| EdgeId::new(id, true))
        }
    }

    /// Adds a new edge and return a pair (created, edge_id) where:
    ///  - created is false if NO new edge was inserted (it was merge with an identical edge already in the DB)
    ///  - edge_id is the id of the edge
    pub fn push_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> (bool, EdgeId) {
        let edge = Edge::new(source, target, weight);
        match self.find_existing(&edge) {
            Some(id) => {
                // edge already exists in the DB, return its id and say it wasn't created
                debug_assert_eq!(self[DirEdge::forward(id)].as_edge(), edge);
                debug_assert_eq!(self[DirEdge::backward(id)].as_edge(), edge);
                (false, id)
            }
            None => {
                // edge does not exist, record the corresponding pair and return the new id.
                let pair = ConstraintPair::new_inactives(edge);
                let base = pair.base_forward.as_edge();
                let id1 = self.constraints.push(pair.base_forward);
                let _ = self.constraints.push(pair.base_backward);
                let id2 = self.constraints.push(pair.negated_forward);
                let _ = self.constraints.push(pair.negated_backward);
                self.lookup.insert(base, id1.edge().base_id());
                debug_assert_eq!(id1.edge().base_id(), id2.edge().base_id());
                let edge_id = if edge.is_negated() { id2 } else { id1 };
                (true, edge_id.edge())
            }
        }
    }

    /// Removes the last created ConstraintPair in the DB. Note that this will remove the last edge that was
    /// pushed and THAT WAS NOT UNIFIED with an existing edge (i.e. edge_push returned : (true, _)).
    pub fn pop_last(&mut self) {
        debug_assert_eq!(self.constraints.len() % 4, 0);
        // remove the four edges (forward and backward) for both the base and negated edge
        self.constraints.pop();
        self.constraints.pop();
        self.constraints.pop();
        if let Some(c) = self.constraints.pop() {
            debug_assert!(c.as_edge().is_canonical());
            self.lookup.remove(&c.as_edge());
        }
    }

    pub fn has_edge(&self, id: EdgeId) -> bool {
        id.base_id() <= self.constraints.len() as u32
    }
}
impl Index<DirEdge> for ConstraintDb {
    type Output = DirConstraint;

    fn index(&self, index: DirEdge) -> &Self::Output {
        &self.constraints[index]
    }
}
impl IndexMut<DirEdge> for ConstraintDb {
    fn index_mut(&mut self, index: DirEdge) -> &mut Self::Output {
        &mut self.constraints[index]
    }
}

type BacktrackLevel = DecLvl;

#[derive(Copy, Clone)]
enum Event {
    EdgeAdded,
    EdgeActivated(DirEdge),
    AddedTheoryPropagationCause,
}

#[derive(Default, Clone)]
struct Stats {
    num_propagations: u64,
    distance_updates: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Identity<Cause>
where
    Cause: From<u32>,
    u32: From<Cause>,
{
    pub(crate) writer_id: WriterId,
    _cause: PhantomData<Cause>,
}

impl<C> Identity<C>
where
    C: From<u32>,
    u32: From<C>,
{
    pub fn new(writer_id: WriterId) -> Self {
        Identity {
            writer_id,
            _cause: Default::default(),
        }
    }

    pub fn inference(&self, cause: C) -> Cause {
        self.writer_id.cause(cause)
    }
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
/// Requirement for weight : a i32 is used internally to represent both delays
/// (weight on edges) and absolute times (bound on nodes). It is the responsibility
/// of the caller to ensure that no overflow occurs when adding an absolute and relative time,
/// either by the choice of an appropriate type (e.g. saturating add) or by the choice of
/// appropriate initial bounds.
#[derive(Clone)]
pub struct StnTheory {
    pub config: StnConfig,
    constraints: ConstraintDb,
    /// Forward/Backward adjacency list containing active edges.
    active_propagators: RefVec<VarBound, Vec<Propagator>>,
    pending_updates: RefSet<VarBound>,
    /// History of changes and made to the STN with all information necessary to undo them.
    trail: Trail<Event>,
    pending_activations: VecDeque<ActivationEvent>,
    stats: Stats,
    pub(crate) identity: Identity<ModelUpdateCause>,
    model_events: ObsTrailCursor<ModelEvent>,
    /// Internal data structure to construct explanations as negative cycles.
    /// When encountering an inconsistency, this vector will be cleared and
    /// a negative cycle will be constructed in it. The explanation returned
    /// will be a slice of this vector to avoid any allocation.
    explanation: Vec<DirEdge>,
    theory_propagation_causes: Vec<TheoryPropagationCause>,
    /// Internal data structure used by the `propagate` method to keep track of pending work.
    internal_propagate_queue: VecDeque<VarBound>,
    /// Internal data structures used for distance computation.
    internal_dijkstra_states: [DijkstraState; 2],
}

/// Indicates the source and target of an active shortest path that caused a propagation
#[derive(Copy, Clone, Debug)]
enum TheoryPropagationCause {
    /// Theory propagation was triggered by a path from source to target in the graph of active constraints
    /// The activation of `triggering_edge` was the one that caused the propagation, meaning that the
    /// shortest path goes through it.
    Path {
        source: VarBound,
        target: VarBound,
        triggering_edge: DirEdge,
    },
    /// Theory propagation was triggered by the incompatibility of the two bounds with an edge in the graph.
    Bounds { source: Lit, target: Lit },
}

#[derive(Copy, Clone)]
pub(crate) enum ModelUpdateCause {
    /// The update was caused by an edge propagation
    EdgePropagation(DirEdge),
    /// index in the trail of the TheoryPropagationCause
    TheoryPropagation(u32),
}

impl From<u32> for ModelUpdateCause {
    fn from(enc: u32) -> Self {
        if (enc & 0x1) == 0 {
            ModelUpdateCause::EdgePropagation(DirEdge::from(enc >> 1))
        } else {
            ModelUpdateCause::TheoryPropagation(enc >> 1)
        }
    }
}

impl From<ModelUpdateCause> for u32 {
    fn from(cause: ModelUpdateCause) -> Self {
        match cause {
            ModelUpdateCause::EdgePropagation(edge) => u32::from(edge) << 1,
            ModelUpdateCause::TheoryPropagation(index) => (index << 1) + 0x1,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Propagator {
    target: VarBound,
    weight: BoundValueAdd,
    id: DirEdge,
}

#[derive(Copy, Clone)]
enum ActivationEvent {
    /// Should activate the given edge, enabled by this literal
    ToActivate(DirEdge, Lit),
}

impl StnTheory {
    /// Creates a new STN. Initially, the STN contains a single timepoint
    /// representing the origin whose domain is [0,0]. The id of this timepoint can
    /// be retrieved with the `origin()` method.
    pub fn new(identity: WriterId, config: StnConfig) -> Self {
        StnTheory {
            config,
            constraints: ConstraintDb::new(),
            active_propagators: Default::default(),
            pending_updates: Default::default(),
            trail: Default::default(),
            pending_activations: VecDeque::new(),
            stats: Default::default(),
            identity: Identity::new(identity),
            model_events: ObsTrailCursor::new(),
            explanation: vec![],
            theory_propagation_causes: Default::default(),
            internal_propagate_queue: Default::default(),
            internal_dijkstra_states: Default::default(),
        }
    }
    pub fn num_nodes(&self) -> u32 {
        (self.active_propagators.len() / 2) as u32
    }

    pub fn reserve_timepoint(&mut self) {
        // add slots for the propagators of both bounds
        self.active_propagators.push(Vec::new());
        self.active_propagators.push(Vec::new());
    }

    pub fn add_reified_edge(
        &mut self,
        literal: Lit,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        model: &Model,
    ) -> EdgeId {
        let e = self.add_inactive_constraint(source.into(), target.into(), weight).0;

        if model.entails(literal) {
            assert_eq!(model.discrete.entailing_level(literal), DecLvl::ROOT);
            self.mark_active(e, literal);
        } else if model.entails(!literal) {
            assert_eq!(model.discrete.entailing_level(!literal), DecLvl::ROOT);
            self.mark_active(!e, !literal);
        } else {
            self.constraints.add_enabler(e, literal);
            self.constraints.add_enabler(!e, !literal);
        }

        e
    }

    /// Adds an edge `source --- weight ---> target` to the network that must hold if
    /// both `source` and `target` are present.
    ///
    /// To control propagation the following literals are provided:
    ///  - `forward_prop`: true if propagation is allowed from `source` to `target`
    ///    This is typically equivalent to   `present(source) => present(target)`
    ///  - `backward_prop`: true if propagation is allowed from `target` to `source`
    ///    This is typically equivalent to   `present(target) => present(source)`
    ///  - `presence`: true if both timepoints are present, and thus the edge is active.
    ///    equivalent to `present(source) and present(target)`. This parameter is optional
    ///    and is used in theory propagation to deactivate the edge.
    #[allow(clippy::too_many_arguments)]
    pub fn add_optional_true_edge(
        &mut self,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        forward_prop: Lit,
        backward_prop: Lit,
        presence: Option<Lit>,
        model: &Model,
    ) -> EdgeId {
        let e = self.add_inactive_constraint(source.into(), target.into(), weight).0;

        self.constraints
            .add_directed_enabler(e.forward(), forward_prop, presence);
        if model.entails(forward_prop) {
            assert_eq!(model.discrete.entailing_level(forward_prop), DecLvl::ROOT);
            self.pending_activations
                .push_back(ActivationEvent::ToActivate(e.forward(), forward_prop));
        }
        self.constraints
            .add_directed_enabler(e.backward(), backward_prop, presence);
        if model.entails(backward_prop) {
            assert_eq!(model.discrete.entailing_level(backward_prop), DecLvl::ROOT);
            self.pending_activations
                .push_back(ActivationEvent::ToActivate(e.backward(), backward_prop));
        }

        e
    }

    /// Marks an edge as active and enqueue it for propagation.
    /// No changes are committed to the network by this function until a call to `propagate_all()`
    pub fn mark_active(&mut self, edge: EdgeId, enabler: Lit) {
        debug_assert!(self.constraints.has_edge(edge));
        self.pending_activations
            .push_back(ActivationEvent::ToActivate(DirEdge::forward(edge), enabler));
        self.pending_activations
            .push_back(ActivationEvent::ToActivate(DirEdge::backward(edge), enabler));
    }

    fn build_contradiction(&self, culprits: &[DirEdge], model: &DiscreteModel) -> Contradiction {
        let mut expl = Explanation::with_capacity(culprits.len());
        for &edge in culprits {
            debug_assert!(self.active(edge));
            let literal = self.constraints[edge].enabler;
            let literal = literal.expect("No entailed enabler for this edge");
            debug_assert!(model.entails(literal));
            expl.push(literal);
        }
        Contradiction::Explanation(expl)
    }

    fn explain_bound_propagation(
        &self,
        event: Lit,
        propagator: DirEdge,
        model: &DiscreteModel,
        out_explanation: &mut Explanation,
    ) {
        debug_assert!(self.active(propagator));
        let c = &self.constraints[propagator];
        let val = event.bound_value();
        debug_assert_eq!(event.affected_bound(), c.target);

        let literal = self.constraints[propagator].enabler.expect("inactive constraint");
        out_explanation.push(literal);

        let cause = Lit::from_parts(c.source, val - c.weight);
        debug_assert!(model.entails(cause));

        if self.config.deep_explanation {
            // function that return the stn propagator responsible for this literal being set,
            // of None if it was not set by a bound propagation of the STN.
            let propagator_of = |lit: Lit, model: &DiscreteModel| -> Option<DirEdge> {
                if let Some(event_index) = model.implying_event(lit) {
                    let event = model.get_event(event_index);
                    match event.cause.as_external_inference() {
                        Some(InferenceCause { writer, payload }) if writer == self.identity.writer_id => {
                            match ModelUpdateCause::from(payload) {
                                ModelUpdateCause::EdgePropagation(edge) => Some(edge),
                                ModelUpdateCause::TheoryPropagation(_) => None,
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            };
            let mut latest_trigger = cause;
            while let Some(propagator) = propagator_of(latest_trigger, model) {
                let c = &self.constraints[propagator];
                let propagator_enabler = c.enabler.expect("inactive edge");
                out_explanation.push(propagator_enabler);
                debug_assert_eq!(latest_trigger.affected_bound(), c.target);
                latest_trigger = Lit::from_parts(c.source, latest_trigger.bound_value() - c.weight);
                debug_assert!(model.entails(latest_trigger));
            }
            out_explanation.push(latest_trigger);
        } else {
            out_explanation.push(cause);
        }
    }

    /// Explains a model update that was caused by theory propagation, either on edge addition or bound update.
    fn explain_theory_propagation(
        &self,
        cause: TheoryPropagationCause,
        model: &DiscreteModel,
        out_explanation: &mut Explanation,
    ) {
        match cause {
            TheoryPropagationCause::Path {
                source,
                target,
                triggering_edge,
            } => {
                let path = self.theory_propagation_path(source, target, triggering_edge, model);

                for edge in path {
                    let literal = self.constraints[edge].enabler.expect("inactive constraint");
                    out_explanation.push(literal);
                }
            }
            TheoryPropagationCause::Bounds { source, target } => {
                debug_assert!(model.entails(source) && model.entails(target));
                out_explanation.push(source);
                out_explanation.push(target);
            }
        }
    }

    /// Propagates all edges that have been marked as active since the last propagation.
    pub fn propagate_all(&mut self, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        // in first propagation, process each edge once to check if it can be added to the model based on the bounds
        // of its extremities. If it is not the case, make its enablers false.
        // This step is equivalent to "bound theory propagation" but need to be made independently because
        // we do not get events for the initial domain of the variables.
        if self.config.theory_propagation.bounds() {
            while let Some(c) = self.constraints.next_new_constraint() {
                // ignore enabled edges, they are dealt with by normal propagation
                if c.enabler.is_none() {
                    let new_lb = model.domains.get_bound(c.source) + c.weight;
                    let current_ub = model.domains.get_bound(c.target.symmetric_bound());
                    if !new_lb.compatible_with_symmetric(current_ub) {
                        // the edge is invalid, build a cause to allow explanation
                        let cause = TheoryPropagationCause::Bounds {
                            source: Lit::from_parts(c.source, model.domains.get_bound(c.source)),
                            target: Lit::from_parts(c.target.symmetric_bound(), current_ub),
                        };
                        let cause_index = self.theory_propagation_causes.len();
                        self.theory_propagation_causes.push(cause);
                        self.trail.push(Event::AddedTheoryPropagationCause);
                        let cause = self
                            .identity
                            .inference(ModelUpdateCause::TheoryPropagation(cause_index as u32));
                        // make all enablers false
                        for &l in &c.enablers {
                            model.domains.set(!l, cause)?;
                        }
                    }
                }
            }
        }

        while self.model_events.num_pending(model.trail()) > 0 || !self.pending_activations.is_empty() {
            // start by propagating all bounds changes before considering the new edges.
            // This is necessary because cycle detection on the insertion of a new edge requires
            // a consistent STN and no interference of external bound updates.
            while let Some(ev) = self.model_events.pop(model.trail()).copied() {
                let literal = ev.new_literal();
                for edge in self.constraints.watches.watches_on(literal) {
                    // mark active
                    debug_assert!(self.constraints.has_edge(edge.edge()));
                    self.pending_activations
                        .push_back(ActivationEvent::ToActivate(edge, literal));
                }
                if self.config.theory_propagation.bounds() {
                    self.theory_propagate_bound(literal, model)?;
                }
                if let Some(x) = ev.cause.as_external_inference() {
                    if x.writer == self.identity.writer_id
                        && matches!(ModelUpdateCause::from(x.payload), ModelUpdateCause::EdgePropagation(_))
                    {
                        // we generated this event ourselves by edge propagation, we can safely ignore it as it would have been handled
                        // immediately
                        continue;
                    }
                }
                self.propagate_bound_change(literal, model)?;
            }
            while let Some(event) = self.pending_activations.pop_front() {
                let ActivationEvent::ToActivate(edge, enabler) = event;
                let c = &mut self.constraints[edge];
                if c.enabler.is_none() {
                    // edge is currently inactive
                    c.enabler = Some(enabler);
                    let c = &self.constraints[edge];
                    if c.source == c.target {
                        // we are in a self loop, that must must handled separately since they are trivial
                        // to handle and not supported by the propagation loop
                        if c.weight.is_tightening() {
                            // negative self loop: inconsistency
                            self.explanation.clear();
                            self.explanation.push(edge);
                            return Err(self.build_contradiction(&self.explanation, model));
                        } else {
                            // positive self loop : useless edge that we can ignore
                        }
                    } else {
                        debug_assert_ne!(c.source, c.target);

                        self.active_propagators[c.source].push(Propagator {
                            target: c.target,
                            weight: c.weight,
                            id: edge,
                        });
                        self.trail.push(EdgeActivated(edge));
                        self.propagate_new_edge(edge, model)?;

                        if self.config.theory_propagation.edges() {
                            self.theory_propagate_edge(edge, model)?;
                        }
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

    /// Undo the last event in the STN, assuming that this would not result in changing the decision level.
    fn undo_last_event(&mut self) {
        // undo changes since the last backtrack point
        let constraints = &mut self.constraints;
        let active_propagators = &mut self.active_propagators;
        let theory_propagation_causes = &mut self.theory_propagation_causes;
        match self.trail.pop_within_level().unwrap() {
            EdgeAdded => constraints.pop_last(),
            EdgeActivated(e) => {
                let c = &mut constraints[e];
                active_propagators[c.source].pop();
                c.enabler = None;
            }
            Event::AddedTheoryPropagationCause => {
                theory_propagation_causes.pop().unwrap();
            }
        };
    }

    pub fn undo_to_last_backtrack_point(&mut self) -> Option<BacktrackLevel> {
        // remove pending activations
        // invariant: there are no pending activation when saving the state
        self.pending_activations.clear();

        // undo changes since the last backtrack point
        let constraints = &mut self.constraints;
        let active_propagators = &mut self.active_propagators;
        let theory_propagation_causes = &mut self.theory_propagation_causes;
        self.trail.restore_last_with(|ev| match ev {
            EdgeAdded => constraints.pop_last(),
            EdgeActivated(e) => {
                let c = &mut constraints[e];
                active_propagators[c.source].pop();
                c.enabler = None;
            }
            Event::AddedTheoryPropagationCause => {
                theory_propagation_causes.pop();
            }
        });

        None
    }

    /// Return a tuple `(id, created)` where id is the id of the edge and created is a boolean value that is true if the
    /// edge was created and false if it was unified with a previous instance
    fn add_inactive_constraint(&mut self, source: Timepoint, target: Timepoint, weight: W) -> (EdgeId, bool) {
        while u32::from(source) >= self.num_nodes() || u32::from(target) >= self.num_nodes() {
            self.reserve_timepoint();
        }
        let (created, id) = self.constraints.push_edge(source, target, weight);
        if created {
            self.trail.push(EdgeAdded);
        }
        (id, created)
    }

    fn active(&self, e: DirEdge) -> bool {
        self.constraints[e].enabler.is_some()
    }

    fn has_edges(&self, var: Timepoint) -> bool {
        u32::from(var) < self.num_nodes()
    }

    /// When a the propagation loops exits with an error (cycle or empty domain),
    /// it might leave its data structures in a dirty state.
    /// This method simply reset it to a pristine state.
    fn clean_up_propagation_state(&mut self) {
        for vb in &self.internal_propagate_queue {
            self.pending_updates.remove(*vb);
        }
        debug_assert!(self.pending_updates.is_empty());
        self.internal_propagate_queue.clear(); // reset to make sure that we are not in a dirty state
    }

    fn propagate_bound_change(&mut self, bound: Lit, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        if !self.has_edges(bound.variable()) {
            return Ok(());
        }
        self.run_propagation_loop(bound.affected_bound(), model, false)
    }

    /// Implementation of [Cesta96]
    /// It propagates a **newly_inserted** edge in a **consistent** STN.
    fn propagate_new_edge(&mut self, new_edge: DirEdge, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        let c = &self.constraints[new_edge];
        debug_assert_ne!(c.source, c.target, "This algorithm does not support self loops.");
        let cause = self.identity.inference(ModelUpdateCause::EdgePropagation(new_edge));
        let source = c.source;
        let target = c.target;
        let weight = c.weight;
        let source_bound = model.domains.get_bound(source);
        if model.domains.set_bound(target, source_bound + weight, cause)? {
            self.run_propagation_loop(target, model, true)?;
        }

        Ok(())
    }

    fn run_propagation_loop(
        &mut self,
        original: VarBound,
        model: &mut DiscreteModel,
        cycle_on_update: bool,
    ) -> Result<(), Contradiction> {
        self.clean_up_propagation_state();
        self.stats.num_propagations += 1;

        self.internal_propagate_queue.push_back(original);
        self.pending_updates.insert(original);

        while let Some(source) = self.internal_propagate_queue.pop_front() {
            let source_bound = model.domains.get_bound(source);
            if !self.pending_updates.contains(source) {
                // bound was already updated
                continue;
            }
            // Remove immediately even if we are not done with update yet
            // This allows to keep the propagation queue and this set in sync:
            // if an element is in this set it also appears in the queue.
            self.pending_updates.remove(source);

            for e in &self.active_propagators[source] {
                let cause = self.identity.inference(ModelUpdateCause::EdgePropagation(e.id));
                let target = e.target;
                debug_assert_ne!(source, target);
                let candidate = source_bound + e.weight;

                if model.domains.set_bound(target, candidate, cause)? {
                    self.stats.distance_updates += 1;
                    if cycle_on_update && target == original {
                        return Err(self.extract_cycle(target, model).into());
                    }
                    self.internal_propagate_queue.push_back(target);
                    self.pending_updates.insert(target);
                }
            }
        }
        Ok(())
    }

    fn extract_cycle(&self, vb: VarBound, model: &DiscreteModel) -> Explanation {
        let mut expl = Explanation::with_capacity(4);
        let mut curr = vb;
        // let mut cycle_length = 0; // TODO: check cycle length in debug
        loop {
            let value = model.domains.get_bound(curr);
            let lit = Lit::from_parts(curr, value);
            debug_assert!(model.entails(lit));
            let ev = model.implying_event(lit).unwrap();
            debug_assert_eq!(model.trail().decision_level(ev), self.trail.current_decision_level());
            let ev = model.get_event(ev);
            let edge = match ev.cause.as_external_inference() {
                Some(cause) => match ModelUpdateCause::from(cause.payload) {
                    ModelUpdateCause::EdgePropagation(edge) => edge,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            let c = &self.constraints[edge];
            curr = c.source;
            // cycle_length += c.edge.weight;
            let trigger = self.constraints[edge].enabler.expect("inactive constraint");
            expl.push(trigger);

            if curr == vb {
                // debug_assert!(cycle_length < 0);
                break expl;
            }
        }
    }

    pub fn print_stats(&self) {
        println!("# nodes: {}", self.num_nodes());
        println!("# constraints: {}", self.constraints.constraints.len());
        println!("# propagations: {}", self.stats.num_propagations);
        println!("# domain updates: {}", self.stats.distance_updates);
    }

    /******** Distances ********/

    /// Perform theory propagation that follows from the addition of a new bound on a variable.
    ///
    /// A bound on X indicates a shortest path `0  ->  X`, where `0` is a virtual timepoint that represents time origin.
    /// For any time point `Y` we also know the length of the shortest path `Y -> 0` (value of the symmetric bound).
    /// Thus we check that for each potential edge `X -> Y` that it would not create a negative cycle `0 -> X -> Y -> 0`.
    /// If that's the case, we disable this edge by setting its enabler to false.
    fn theory_propagate_bound(&mut self, bound: Lit, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        fn dist_to_origin(bound: Lit) -> BoundValueAdd {
            let x = bound.affected_bound();
            let origin = if x.is_ub() {
                Lit::from_parts(x, BoundValue::ub(0))
            } else {
                Lit::from_parts(x, BoundValue::lb(0))
            };
            bound.bound_value() - origin.bound_value()
        }
        let x = bound.affected_bound();
        let dist_o_x = dist_to_origin(bound);

        for out in self.constraints.potential_out_edges(x) {
            if !model.entails(!out.presence) {
                let y = out.target;
                let w = out.weight;
                let y_sym = y.symmetric_bound();
                let y_sym = y_sym.bind(model.domains.get_bound(y_sym));
                let dist_y_o = dist_to_origin(y_sym);

                let cycle_length = dist_o_x + w + dist_y_o;

                if cycle_length.raw_value() < 0 {
                    // Record the cause so that we can retrieve it if an explanation is needed.
                    // The update of `bound` triggered the propagation. However it is possible that
                    // a less constraining bound would have triggered the propagation as well.
                    // We thus replace `bound` with the smallest update that would have triggered the propagation.
                    // The consequence is that the clauses inferred through explanation will be stronger.
                    let relaxed_bound = Lit::from_parts(
                        bound.affected_bound(),
                        bound.bound_value() - cycle_length - BoundValueAdd::on_ub(1),
                    );
                    // check that the relaxed bound would have triggered a propagation with teh cycle having exactly length -1
                    debug_assert_eq!((dist_to_origin(relaxed_bound) + w + dist_y_o).raw_value(), -1);
                    let cause = TheoryPropagationCause::Bounds {
                        source: relaxed_bound,
                        target: y_sym,
                    };
                    let cause_index = self.theory_propagation_causes.len();
                    self.theory_propagation_causes.push(cause);
                    self.trail.push(Event::AddedTheoryPropagationCause);
                    let cause = self
                        .identity
                        .inference(ModelUpdateCause::TheoryPropagation(cause_index as u32));

                    // disable the edge
                    model.domains.set(!out.presence, cause)?;
                }
            }
        }
        Ok(())
    }

    /// Perform the theory propagation that follows from the addition of the given edge.
    ///
    /// In essence, we find all shortest paths A -> B that contain the new edge.
    /// Then we check if there exist an inactive edge BA where `weight(BA) + dist(AB) < 0`.
    /// For each such edge, we set its enabler to false since its addition would result in a negative cycle.
    fn theory_propagate_edge(&mut self, edge: DirEdge, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        let constraint = &self.constraints[edge];
        let target = constraint.target;
        let source = constraint.source;

        // get ownership of data structures used by dijkstra's algorithm.
        // we let empty place holder that will need to be swapped back.
        let mut successors = DijkstraState::default();
        let mut predecessors = DijkstraState::default();
        std::mem::swap(&mut successors, &mut self.internal_dijkstra_states[0]);
        std::mem::swap(&mut predecessors, &mut self.internal_dijkstra_states[1]);

        // find all nodes reachable from target(edge), including itself
        self.distances_from(target, model, &mut successors);

        // find all nodes that can reach source(edge), including itself
        // predecessors nodes and edge are in the inverse direction
        self.distances_from(source.symmetric_bound(), model, &mut predecessors);

        // iterate through all predecessors, they will constitute the source of our shortest paths
        let mut predecessor_entries = predecessors.distances();
        while let Some((pred, pred_dist)) = predecessor_entries.next() {
            // find all potential edges that target this predecessor.
            // note that the predecessor is the inverse view (symmetric_bound); hence the potential out_edge are all
            // inverse edges
            for potential in self.constraints.potential_out_edges(pred) {
                // potential is an edge `X -> pred`
                // do we have X in the successors ?
                if let Some(forward_dist) = successors.distance(potential.target.symmetric_bound()) {
                    let back_dist = pred_dist + potential.weight;
                    let total_dist = back_dist + constraint.weight + forward_dist;

                    let real_dist = total_dist.raw_value();
                    if real_dist < 0 && !model.domains.entails(!potential.presence) {
                        // this edge would be violated and is not inactive yet

                        // careful: we are doing batched eager updates involving optional variable
                        // When doing the shortest path computation, we followed any edge that was not
                        // proven inactive yet.
                        // The current theory propagation, might have been preceded by an other affecting the network.
                        // Here we thus check that the path we initially computed is still active, i.e., that
                        // no other propagation ame any of its edges inactive.
                        // This is necessary because we need to be able to explain any change and explanation
                        // would not follow any inactive edge when recreating the path.
                        let active = self.theory_propagation_path_active(
                            pred.symmetric_bound(),
                            potential.target.symmetric_bound(),
                            edge,
                            model,
                            &successors,
                            &predecessors,
                        );
                        if !active {
                            // the shortest path was made inactive, ignore this update
                            // Note that on a valid constraint network, making this change should be
                            // either a noop or redundant with another constraint.
                            continue;
                        }

                        // record the cause so that we can explain the model's change
                        let cause = TheoryPropagationCause::Path {
                            source: pred.symmetric_bound(),
                            target: potential.target.symmetric_bound(),
                            triggering_edge: edge,
                        };
                        let cause_index = self.theory_propagation_causes.len();
                        self.theory_propagation_causes.push(cause);
                        self.trail.push(Event::AddedTheoryPropagationCause);

                        // update the model to force this edge to be inactive
                        if let Err(x) = model.domains.set(
                            !potential.presence,
                            self.identity
                                .inference(ModelUpdateCause::TheoryPropagation(cause_index as u32)),
                        ) {
                            // inconsistent model after propagation,
                            // restore the dijkstra state entries for future use
                            std::mem::forget(predecessor_entries);
                            self.internal_dijkstra_states[0] = successors;
                            self.internal_dijkstra_states[1] = predecessors;
                            return Err(x.into());
                        }
                    }
                }
            }
        }
        // restore the dijkstra state entries for future use
        std::mem::forget(predecessor_entries);
        self.internal_dijkstra_states[0] = successors;
        self.internal_dijkstra_states[1] = predecessors;

        // finished propagation without any inconsistency
        Ok(())
    }

    /// This method checks whether the `theory_propagation_path` method would be able to find a path
    /// for explaining a theory propagation.
    ///
    /// For efficiency reasons, we do not run the dijkstra algorithm.
    /// Instead we accept two prefilled Dijkstra state:
    ///   - `successors`: one-to-all distances from `through_edge.target`
    ///   - `predecessors`: one-to-all distances from `through_edge.source.symmetric_bound`
    /// Complexity is linear in the length of the path to check.
    fn theory_propagation_path_active(
        &self,
        source: VarBound,
        target: VarBound,
        through_edge: DirEdge,
        model: &DiscreteModel,
        successors: &DijkstraState,
        predecessors: &DijkstraState,
    ) -> bool {
        let e = &self.constraints[through_edge];

        // A path is active (i.e. findable by Dijkstra) if all nodes in it are not shown
        // absent.
        // We assume that the edges themselves are active (since it cannot be made inactive once activated).
        let path_active = |src: VarBound, tgt: VarBound, dij: &DijkstraState| {
            debug_assert!(dij.distance(tgt).is_some());
            let mut curr = tgt;
            if model.domains.present(curr.variable()) == Some(false) {
                return false;
            }
            while curr != src {
                let pred_edge = dij.predecessor(curr).unwrap();
                let e = &self.constraints[pred_edge];
                debug_assert!(e.enabler.is_some());
                curr = e.source;
                if model.domains.present(curr.variable()) == Some(false) {
                    return false;
                }
            }
            true
        };

        // the path is active if both its prefix and its postfix are active.
        let active = path_active(e.target, target, successors)
            && path_active(e.source.symmetric_bound(), source.symmetric_bound(), predecessors);

        debug_assert!(
            !active || {
                self.theory_propagation_path(source, target, through_edge, model);
                true
            },
            "A panic indicates that we were unable to reconstruct the path, meaning this implementation is invalid."
        );

        active
    }

    pub fn forward_dist(&self, var: VarRef, model: &DiscreteModel) -> RefMap<VarRef, W> {
        let mut dists = DijkstraState::default();
        self.distances_from(VarBound::ub(var), model, &mut dists);
        dists.distances().map(|(v, d)| (v.variable(), d.as_ub_add())).collect()
    }

    pub fn backward_dist(&self, var: VarRef, model: &DiscreteModel) -> RefMap<VarRef, W> {
        let mut dists = DijkstraState::default();
        self.distances_from(VarBound::lb(var), model, &mut dists);
        dists.distances().map(|(v, d)| (v.variable(), d.as_lb_add())).collect()
    }

    /// Computes the one-to-all shortest paths in an STN.
    /// The shortest paths are:
    ///  - in the forward graph if the origin is the upper bound of a variable
    ///  - in the backward graph is the origin is the lower bound of a variable
    ///
    /// The functions expects a `state` parameter that will be cleared and contains datastructures
    /// that will be used to compute the result. The distances will be set in the `distances` field
    /// of this state.
    ///
    /// The distances returned are in the [BoundValueAdd] format, which is agnostic of whether we are
    /// computing backward or forward distances.
    /// The returned distance to a node `A` are simply the sum of the edge weights over the shortest path.
    ///
    /// # Assumptions
    ///
    /// The STN is consistent and fully propagated.
    ///
    /// # Internals
    ///
    /// To use Dijkstra's algorithm, we need to ensure that all edges are positive.
    /// We do this by using the reduced costs of the edges.
    /// Given a function `value(VarBound)` that returns the current value of a variable bound, we define the
    /// *reduced distance* `red_dist` of a path `source -- dist --> target`  as   
    ///   - `red_dist = dist - value(target) + value(source)`
    ///   - `dist = red_dist + value(target) - value(source)`
    /// If the STN is fully propagated and consistent, the reduced distance is guaranteed to always be positive.
    fn distances_from(&self, origin: VarBound, model: &DiscreteModel, state: &mut DijkstraState) {
        let origin_bound = model.domains.get_bound(origin);

        state.clear();
        state.enqueue(origin, BoundValueAdd::ZERO, None);

        // run dijkstra until exhaustion to find all reachable nodes
        self.run_dijkstra(model, state, |_| false);

        // convert all reduced distances to true distances.
        for (curr_node, (dist, _)) in state.distances.entries_mut() {
            let curr_bound = model.domains.get_bound(curr_node);
            let true_distance = *dist + (curr_bound - origin_bound);
            *dist = true_distance
        }
    }

    /// Appends to `out` a set of edges that constitute a shortest path from `from` to `to`.
    /// The edges are append in no particular order.
    ///
    /// The `state` parameter is provided to avoid allocating memory and will be cleared before usage.
    fn shortest_path_from_to(
        &self,
        from: VarBound,
        to: VarBound,
        model: &DiscreteModel,
        state: &mut DijkstraState,
        out: &mut Vec<DirEdge>,
    ) {
        state.clear();
        state.enqueue(from, BoundValueAdd::ZERO, None);

        // run dijkstra until exhaustion to find all reachable nodes
        self.run_dijkstra(model, state, |curr| curr == to);

        // go up the predecessors chain to extract the shortest path and append the edge to `out`
        let mut curr = to;
        while curr != from {
            let edge = state.predecessor(curr).unwrap();
            out.push(edge);
            debug_assert_eq!(self.constraints[edge].target, curr);
            curr = self.constraints[edge].source;
        }
    }

    /// Run the Dijkstra algorithm from a pre-initialized queue.
    /// The queue should initially contain the origin of the shortest path problem.
    /// The algorithm will once the queue is exhausted or the predicate `stop` returns true when given
    /// the next node to expand.
    ///
    /// At the end of the method, the `state` will contain the distances and predecessors of all nodes
    /// reached by the algorithm.
    fn run_dijkstra(&self, model: &DiscreteModel, state: &mut DijkstraState, stop: impl Fn(VarBound) -> bool) {
        while let Some((curr_node, curr_rdist)) = state.dequeue() {
            if stop(curr_node) {
                return;
            }
            if model.domains.present(curr_node.variable()) == Some(false) {
                continue;
            }
            let curr_bound = model.domains.get_bound(curr_node);

            // process all outgoing edges
            for prop in &self.active_propagators[curr_node] {
                if !state.is_final(prop.target) && model.domains.present(prop.target.variable()) != Some(false) {
                    // we do not have a shortest path to this node yet.
                    // compute the reduced_cost of the the edge
                    let target_bound = model.domains.get_bound(prop.target);
                    let cost = prop.weight;
                    // rcost(curr, tgt) = cost(curr, tgt) + val(curr) - val(tgt)
                    let reduced_cost = cost + (curr_bound - target_bound);

                    debug_assert!(reduced_cost.raw_value() >= 0);

                    // rdist(orig, tgt) = dist(orig, tgt) +  val(tgt) - val(orig)
                    //                  = dist(orig, curr) + cost(curr, tgt) + val(tgt) - val(orig)
                    //                  = [rdist(orig, curr) + val(orig) - val(curr)] + [rcost(curr, tgt) - val(tgt) + val(curr)] + val(tgt) - val(orig)
                    //                  = rdist(orig, curr) + rcost(curr, tgt)
                    let reduced_dist = curr_rdist + reduced_cost;

                    state.enqueue(prop.target, reduced_dist, Some(prop.id));
                }
            }
        }
    }

    /// Reconstructs a path that triggered a theory propagation.
    /// It is a shortest path from `source` to `target` that goes through `through_edge`.
    ///
    /// The theory propagation was initially triggered by the activation of `through_edge` and
    /// the resulting path was conflicting with an edge `target -> source` that would have form
    /// a negative cycle if activated.
    fn theory_propagation_path(
        &self,
        source: VarBound,
        target: VarBound,
        through_edge: DirEdge,
        model: &DiscreteModel,
    ) -> Vec<DirEdge> {
        let mut path = Vec::with_capacity(8);

        let e = &self.constraints[through_edge];
        let mut dij = DijkstraState::default();

        //add `e.source -> e.target` edge to path
        path.push(through_edge);

        // add `e.target ----> target` subpath to path
        self.shortest_path_from_to(e.target, target, model, &mut dij, &mut path);
        // add `source ----> e.source` subpath to path, computed in the reverse direction
        self.shortest_path_from_to(
            e.source.symmetric_bound(),
            source.symmetric_bound(),
            model,
            &mut dij,
            &mut path,
        );

        path
    }
}

impl Theory for StnTheory {
    fn identity(&self) -> WriterId {
        self.identity.writer_id
    }

    fn bind(
        &mut self,
        literal: Lit,
        expr: ExprHandle,
        model: &mut Model,
        queue: &mut ObsTrail<Binding>,
    ) -> BindingResult {
        let expr = model.expressions.get(expr);

        // function that transforms the parameters into two `IAtom`s, panicking if it is not possible
        let args_as_two_integers = || {
            assert_eq!(expr.args.len(), 2);
            let a = IAtom::try_from(expr.args[0]).expect("type error");
            let b = IAtom::try_from(expr.args[1]).expect("type error");
            (a, b)
        };
        // function that extracts the variable inside an IAtom, panicking if it is not possible
        let var_in = |a: IAtom| match a.var {
            Some(v) => v,
            None => panic!("leq with no variable on the left side"),
        };

        match expr.fun {
            Fun::Leq => {
                let (a, b) = args_as_two_integers();
                let va = var_in(a);
                let vb = var_in(b);

                // va + da <= vb + db    <=>   va - vb <= db - da
                self.add_reified_edge(literal, vb, va, b.shift - a.shift, model);

                BindingResult::Enforced
            }
            Fun::Eq => {
                let (a, b) = args_as_two_integers();
                let x = model.leq(a, b);
                let y = model.leq(b, a);
                queue.push(Binding::new(literal, model.and2(x, y))); // TODO: we can split this if know the value of literal
                BindingResult::Refined
            }
            Fun::OptEq if literal == Lit::TRUE => {
                let (a, b) = args_as_two_integers();

                debug_assert!(literal == Lit::TRUE, "Assumed for posting the two LEQ constraints");
                queue.push(Binding::new(literal, model.opt_leq(a, b)));
                queue.push(Binding::new(literal, model.opt_leq(b, a)));
                BindingResult::Refined
            }
            Fun::OptLeq if literal == Lit::TRUE => {
                let (a, b) = args_as_two_integers();
                let va = var_in(a);
                let vb = var_in(b);

                // va + da <= vb + db    <=>   va - vb <= db - da
                let delay = b.shift - a.shift;
                let a = va.into();
                let b = vb.into();

                let a_to_b = can_propagate(&model.discrete.domains, a, b);
                let b_to_a = can_propagate(&model.discrete.domains, b, a);
                let presence = edge_presence(&model.discrete.domains, a, b);
                self.add_optional_true_edge(b, a, delay, b_to_a, a_to_b, presence, model);
                BindingResult::Enforced
            }
            Fun::OptLeq if literal == Lit::FALSE => {
                // this constraint is always false, post the opposite
                let (a, b) = args_as_two_integers();
                let va = var_in(a);
                let vb = var_in(b);

                // va + da <= vb + db    <=>   va - vb <= db - da
                let delay = a.shift - b.shift - 1;
                let a = vb.into();
                let b = va.into();

                let a_to_b = can_propagate(&model.discrete.domains, a, b);
                let b_to_a = can_propagate(&model.discrete.domains, b, a);
                let presence = edge_presence(&model.discrete.domains, a, b);
                self.add_optional_true_edge(b, a, delay, b_to_a, a_to_b, presence, model);
                BindingResult::Enforced
            }

            _ => BindingResult::Unsupported,
        }
    }

    fn propagate(&mut self, model: &mut DiscreteModel) -> Result<(), Contradiction> {
        self.propagate_all(model)
    }

    fn explain(&mut self, event: Lit, context: u32, model: &DiscreteModel, out_explanation: &mut Explanation) {
        match ModelUpdateCause::from(context) {
            ModelUpdateCause::EdgePropagation(edge_id) => {
                self.explain_bound_propagation(event, edge_id, model, out_explanation)
            }
            ModelUpdateCause::TheoryPropagation(cause_index) => {
                let cause = self.theory_propagation_causes[cause_index as usize];
                // We need to replace ourselves in exactly the context in which this theory propagation occurred.
                // Undo all events until we are back in the state where this theory propagation cause
                // had not occurred yet.
                while (cause_index as usize) < self.theory_propagation_causes.len() {
                    self.undo_last_event();
                }
                self.explain_theory_propagation(cause, model, out_explanation)
            }
        }
    }

    fn print_stats(&self) {
        self.print_stats()
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for StnTheory {
    fn save_state(&mut self) -> DecLvl {
        self.set_backtrack_point()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.undo_to_last_backtrack_point();
    }
}

/// Returns the literal that is true if propagation is allowed from one optional variable to another.
///
/// # Panics
/// If the the two variable are in unrelated scopes.
pub(crate) fn can_propagate(doms: &OptDomains, source: Timepoint, target: Timepoint) -> Lit {
    // lit = (from ---> to)    ,  we can if (lit != false) && p(from) => p(to)
    if doms.only_present_with(target, source) {
        // p(target) => p(source)
        // !p(source) => !p(target)
        Lit::TRUE
    } else if doms.only_present_with(source, target) {
        // p(source) => p(target)
        doms.presence(source)
    } else {
        panic!()
    }
}

/// Returns a literal that is true iff both optional variables are present.
/// Returns None if it was not possible to find such a literal
pub(crate) fn edge_presence(doms: &OptDomains, var1: Timepoint, var2: Timepoint) -> Option<Lit> {
    if doms.only_present_with(var2, var1) {
        // p(var2) => p(var1)
        Some(doms.presence(var2))
    } else if doms.only_present_with(var1, var2) {
        // p(var1) => p(var2)
        Some(doms.presence(var1))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stn::Stn;
    use aries_model::lang::IVar;

    #[test]
    fn test_edge_id_conversions() {
        fn check_rountrip(i: u32) {
            let edge_id = EdgeId::from(i);
            let i_new = u32::from(edge_id);
            assert_eq!(i, i_new);
            let edge_id_new = EdgeId::from(i_new);
            assert_eq!(edge_id, edge_id_new);
        }

        // check_rountrip(0);
        check_rountrip(1);
        check_rountrip(2);
        check_rountrip(3);
        check_rountrip(4);

        fn check_rountrip2(edge_id: EdgeId) {
            let i = u32::from(edge_id);
            let edge_id_new = EdgeId::from(i);
            assert_eq!(edge_id, edge_id_new);
        }
        check_rountrip2(EdgeId::new(0, true));
        check_rountrip2(EdgeId::new(0, false));
        check_rountrip2(EdgeId::new(1, true));
        check_rountrip2(EdgeId::new(1, false));
    }

    #[test]
    fn test_propagation() {
        let s = &mut Stn::new();
        let a = s.add_timepoint(0, 10);
        let b = s.add_timepoint(0, 10);

        let assert_bounds = |stn: &Stn, a_lb, a_ub, b_lb, b_ub| {
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
        let s = &mut Stn::new();
        let a = s.add_timepoint(0, 10);
        let b = s.add_timepoint(0, 10);

        let assert_bounds = |stn: &Stn, a_lb, a_ub, b_lb, b_ub| {
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
        let mut stn = Stn::new();
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
    fn test_explanation() -> Result<(), Contradiction> {
        let stn = &mut Stn::new();
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);
        let c = stn.add_timepoint(0, 10);
        stn.propagate_all()?;

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

        Ok(())
    }

    #[test]
    fn test_optionals() -> Result<(), Contradiction> {
        let stn = &mut Stn::new();
        let prez_a = stn.model.new_bvar("prez_a").true_lit();
        let a = stn.model.new_optional_ivar(0, 10, prez_a, "a");
        let prez_b = stn.model.new_presence_variable(prez_a, "prez_b").true_lit();
        let b = stn.model.new_optional_ivar(0, 10, prez_b, "b");

        let a_implies_b = prez_b;
        let b_implies_a = Lit::TRUE;
        stn.add_optional_true_edge(b, a, 0, a_implies_b, b_implies_a, Some(a_implies_b));

        stn.propagate_all()?;
        stn.model.discrete.set_lb(b, 1, Cause::Decision)?;
        stn.model.discrete.set_ub(b, 9, Cause::Decision)?;

        stn.propagate_all()?;
        assert_eq!(stn.model.domain_of(a), (0, 10));
        assert_eq!(stn.model.domain_of(b), (1, 9));

        stn.model.discrete.set_lb(a, 2, Cause::Decision)?;

        stn.propagate_all()?;
        assert_eq!(stn.model.domain_of(a), (2, 10));
        assert_eq!(stn.model.domain_of(b), (2, 9));

        stn.model.discrete.domains.set(prez_b, Cause::Decision)?;

        stn.propagate_all()?;
        assert_eq!(stn.model.domain_of(a), (2, 9));
        assert_eq!(stn.model.domain_of(b), (2, 9));

        Ok(())
    }

    #[test]
    fn test_optional_chain() -> Result<(), Contradiction> {
        let stn = &mut Stn::new();
        let mut vars: Vec<(Lit, IVar)> = Vec::new();
        let mut context = Lit::TRUE;
        for i in 0..10 {
            let prez = stn
                .model
                .new_presence_variable(context, format!("prez_{}", i))
                .true_lit();
            let var = stn.model.new_optional_ivar(0, 20, prez, format!("var_{}", i));
            if i > 0 {
                stn.add_delay(vars[i - 1].1.into(), var.into(), 1);
            }
            vars.push((prez, var));
            context = prez;
        }

        stn.propagate_all()?;
        for (i, (_prez, var)) in vars.iter().enumerate() {
            let i = i as i32;
            assert_eq!(stn.model.bounds(*var), (i, 20));
        }
        stn.model.discrete.set_ub(vars[5].1, 4, Cause::Decision)?;
        stn.propagate_all()?;
        for (i, (_prez, var)) in vars.iter().enumerate() {
            let i = i as i32;
            if i <= 4 {
                assert_eq!(stn.model.bounds(*var), (i, 20));
            } else {
                assert_eq!(stn.model.discrete.domains.present((*var).into()), Some(false))
            }
        }

        Ok(())
    }

    #[test]
    fn test_theory_propagation_edges_simple() -> Result<(), Contradiction> {
        let stn = &mut Stn::with_config(StnConfig {
            theory_propagation: TheoryPropagationLevel::Edges,
            ..Default::default()
        });
        let a = stn.model.new_ivar(10, 20, "a").into();
        let prez_a1 = stn.model.new_bvar("prez_a1").true_lit();
        let a1 = stn.model.new_optional_ivar(0, 30, prez_a1, "a1").into();

        stn.add_delay(a, a1, 0);
        stn.add_delay(a1, a, 0);

        let b = stn.model.new_ivar(10, 20, "b").into();
        let prez_b1 = stn.model.new_bvar("prez_b1").true_lit();
        let b1 = stn.model.new_optional_ivar(0, 30, prez_b1, "b1").into();

        stn.add_delay(b, b1, 0);
        stn.add_delay(b1, b, 0);

        // a strictly before b
        let top = stn.add_inactive_edge(b, a, -1);
        // b1 strictly before a1
        let bottom = stn.add_inactive_edge(a1, b1, -1);

        stn.propagate_all()?;
        assert_eq!(stn.model.discrete.domain_of(a1), (10, 20));
        assert_eq!(stn.model.discrete.domain_of(b1), (10, 20));
        stn.model.discrete.domains.set(top, Cause::Decision)?;
        stn.propagate_all()?;

        assert!(stn.model.entails(!bottom));

        Ok(())
    }

    #[test]
    fn test_distances() -> Result<(), Contradiction> {
        let stn = &mut Stn::new();

        // create an STN graph with the following edges, all with a weight of 1
        // A ---> C ---> D ---> E ---> F
        // |                    ^
        // --------- B ----------
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);
        let c = stn.add_timepoint(0, 10);
        let d = stn.add_timepoint(0, 10);
        let e = stn.add_timepoint(0, 10);
        let f = stn.add_timepoint(0, 10);
        stn.add_edge(a, b, 1);
        stn.add_edge(a, c, 1);
        stn.add_edge(c, d, 1);
        stn.add_edge(b, e, 1);
        stn.add_edge(d, e, 1);
        stn.add_edge(e, f, 1);

        stn.propagate_all()?;

        let dists = stn.stn.forward_dist(a, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 6);
        assert_eq!(dists[a], 0);
        assert_eq!(dists[b], 1);
        assert_eq!(dists[c], 1);
        assert_eq!(dists[d], 2);
        assert_eq!(dists[e], 2);
        assert_eq!(dists[f], 3);

        let dists = stn.stn.backward_dist(a, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 1);
        assert_eq!(dists[a], 0);

        let dists = stn.stn.backward_dist(f, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 6);
        assert_eq!(dists[f], 0);
        assert_eq!(dists[e], -1);
        assert_eq!(dists[d], -2);
        assert_eq!(dists[b], -2);
        assert_eq!(dists[c], -3);
        assert_eq!(dists[a], -3);

        let dists = stn.stn.backward_dist(d, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 3);
        assert_eq!(dists[d], 0);
        assert_eq!(dists[c], -1);
        assert_eq!(dists[a], -2);

        Ok(())
    }

    #[test]
    fn test_negative_self_loop() {
        let stn = &mut Stn::new();

        // create an STN graph with the following edges, all with a weight of 1
        // A ---> C ---> D ---> E ---> F
        // |                    ^
        // --------- B ----------
        let a = stn.add_timepoint(0, 1);
        stn.add_edge(a, a, -1);
        assert!(stn.propagate_all().is_err());
    }

    #[test]
    fn test_distances_simple() -> Result<(), Contradiction> {
        let stn = &mut Stn::new();

        // create an STN graph with the following edges, all with a weight of 1
        // A ---> C ---> D ---> E ---> F
        // |                    ^
        // --------- B ----------
        let a = stn.add_timepoint(0, 1);
        let b = stn.add_timepoint(0, 10);
        stn.add_edge(b, a, -1);
        stn.propagate_all()?;

        let dists = stn.stn.backward_dist(a, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 2);
        assert_eq!(dists[a], 0);
        assert_eq!(dists[b], 1);

        Ok(())
    }

    #[test]
    fn test_distances_negative() -> Result<(), Contradiction> {
        let stn = &mut Stn::new();

        // create an STN graph with the following edges, all with a weight of -1
        // A ---> C ---> D ---> E ---> F
        // |                    ^
        // --------- B ----------
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);
        let c = stn.add_timepoint(0, 10);
        let d = stn.add_timepoint(0, 10);
        let e = stn.add_timepoint(0, 10);
        let f = stn.add_timepoint(0, 10);
        stn.add_edge(a, b, -1);
        stn.add_edge(a, c, -1);
        stn.add_edge(c, d, -1);
        stn.add_edge(b, e, -1);
        stn.add_edge(d, e, -1);
        stn.add_edge(e, f, -1);

        stn.propagate_all()?;

        let dists = stn.stn.forward_dist(a, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 6);
        assert_eq!(dists[a], 0);
        assert_eq!(dists[b], -1);
        assert_eq!(dists[c], -1);
        assert_eq!(dists[d], -2);
        assert_eq!(dists[e], -3);
        assert_eq!(dists[f], -4);

        let dists = stn.stn.backward_dist(a, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 1);
        assert_eq!(dists[a], 0);

        let dists = stn.stn.backward_dist(f, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 6);
        assert_eq!(dists[f], 0);
        assert_eq!(dists[e], 1);
        assert_eq!(dists[d], 2);
        assert_eq!(dists[b], 2);
        assert_eq!(dists[c], 3);
        assert_eq!(dists[a], 4);

        let dists = stn.stn.backward_dist(d, &stn.model.discrete);
        assert_eq!(dists.entries().count(), 3);
        assert_eq!(dists[d], 0);
        assert_eq!(dists[c], 1);
        assert_eq!(dists[a], 2);

        Ok(())
    }

    #[test]
    fn test_theory_propagation_edges() -> Result<(), Contradiction> {
        let stn = &mut Stn::with_config(StnConfig {
            theory_propagation: TheoryPropagationLevel::Edges,
            ..Default::default()
        });
        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(0, 10);

        // let d = stn.add_timepoint(0, 10);
        // let e = stn.add_timepoint(0, 10);
        // let f = stn.add_timepoint(0, 10);
        stn.add_edge(a, b, 1);
        let ba0 = stn.add_inactive_edge(b, a, 0);
        let ba1 = stn.add_inactive_edge(b, a, -1);
        let ba2 = stn.add_inactive_edge(b, a, -2);

        assert_eq!(stn.model.discrete.value(ba0), None);
        stn.propagate_all()?;
        assert_eq!(stn.model.discrete.value(ba0), None);
        assert_eq!(stn.model.discrete.value(ba1), None);
        assert_eq!(stn.model.discrete.value(ba2), Some(false));

        let exp = stn.explain_literal(!ba2);
        assert!(exp.literals().is_empty());

        // TODO: adding a new edge does not trigger theory propagation
        // let ba3 = stn.add_inactive_edge(b, a, -3);
        // stn.propagate_all();
        // assert_eq!(stn.model.discrete.value(ba3), Some(false));

        let c = stn.add_timepoint(0, 10);
        let d = stn.add_timepoint(0, 10);
        let e = stn.add_timepoint(0, 10);
        let f = stn.add_timepoint(0, 10);
        let g = stn.add_timepoint(0, 10);

        // create a chain "abcdefg" of length 6
        // the edge in the middle is the last one added
        stn.add_edge(b, c, 1);
        stn.add_edge(c, d, 1);
        let de = stn.add_inactive_edge(d, e, 1);
        stn.add_edge(e, f, 1);
        stn.add_edge(f, g, 1);

        // do not mark active at the root, otherwise the constraint might be inferred as always active
        // its enabler ignored in explanations
        stn.propagate_all()?;
        stn.set_backtrack_point();
        stn.mark_active(de);

        let ga0 = stn.add_inactive_edge(g, a, -5);
        let ga1 = stn.add_inactive_edge(g, a, -6);
        let ga2 = stn.add_inactive_edge(g, a, -7);

        stn.propagate_all()?;
        assert_eq!(stn.model.discrete.value(ga0), None);
        assert_eq!(stn.model.discrete.value(ga1), None);
        assert_eq!(stn.model.discrete.value(ga2), Some(false));

        let exp = stn.explain_literal(!ga2);
        assert_eq!(exp.len(), 1);

        Ok(())
    }

    #[test]
    fn test_theory_propagation_bounds() -> Result<(), Contradiction> {
        let stn = &mut Stn::with_config(StnConfig {
            theory_propagation: TheoryPropagationLevel::Bounds,
            ..Default::default()
        });

        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(10, 20);

        // inactive edge stating that  b <= a
        let edge_trigger = stn.add_inactive_edge(a, b, 0);
        stn.propagate_all()?;
        assert_eq!(stn.model.discrete.value(edge_trigger), None);

        stn.set_backtrack_point();
        stn.model.discrete.set_lb(b, 11, Cause::Decision)?;
        stn.propagate_all()?;
        assert_eq!(stn.model.discrete.value(edge_trigger), Some(false));

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        stn.model.discrete.set_ub(a, 9, Cause::Decision)?;
        stn.propagate_all()?;
        assert_eq!(stn.model.discrete.value(edge_trigger), Some(false));

        Ok(())
    }
}
