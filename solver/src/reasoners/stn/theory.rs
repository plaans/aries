mod bound_propagation;
mod contraint_db;
mod distances;
mod edges;

use crate::backtrack::{Backtrack, EventIndex};
use crate::backtrack::{DecLvl, ObsTrailCursor, Trail};
use crate::collections::ref_store::{RefMap, RefVec};
use crate::core::state::*;
use crate::core::*;
use crate::reasoners::stn::theory::Event::EdgeActivated;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use contraint_db::*;
use distances::{Graph, StnGraph};
use edges::*;
use env_param::EnvParam;
use itertools::Itertools;
use std::collections::VecDeque;
use std::convert::*;
use std::marker::PhantomData;
use std::str::FromStr;

type ModelEvent = crate::core::state::Event;

/// A temporal reference in an STN, i.e., reference to an absolute time.
pub type Timepoint = VarRef;

/// The edge weight of an STN, i.e., a fixed duration.
pub type W = IntCst;

pub static STN_THEORY_PROPAGATION: EnvParam<TheoryPropagationLevel> =
    EnvParam::new("ARIES_STN_THEORY_PROPAGATION", "bounds");
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
                "Unknown theory propagation level: {x}. Valid options: none, literals, edges, full"
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
    /// If true, extensive and very expensive tests will be made in debug mode.
    pub extensive_tests: bool,
}

impl Default for StnConfig {
    fn default() -> Self {
        StnConfig {
            theory_propagation: STN_THEORY_PROPAGATION.get(),
            extensive_tests: STN_EXTENSIVE_TESTS.get(),
        }
    }
}

type BacktrackLevel = DecLvl;

#[derive(Copy, Clone)]
enum Event {
    EdgeActivated(PropagatorId),
    EdgeUpdated {
        prop: PropagatorId,
        previous_weight: IntCst,
        previous_enabler: Option<(Enabler, EventIndex)>,
    },
}

#[derive(Default, Clone)]
struct Stats {
    num_propagations: u64,
    bound_updates: u64,
    num_bound_edge_deactivation: u64,
    num_theory_propagations: u64,
    num_theory_deactivations: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Identity<Cause>
where
    Cause: From<u32>,
    u32: From<Cause>,
{
    pub(crate) writer_id: ReasonerId,
    _cause: PhantomData<Cause>,
}

impl<C> Identity<C>
where
    C: From<u32>,
    u32: From<C>,
{
    pub fn new(writer_id: ReasonerId) -> Self {
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
///  - incremental edge addition and consistency checking with @Cesta96
///  - undoing the latest changes
///  - providing explanation on inconsistency in the form of a culprit
///    set of constraints
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
/// appropriate initial literals.
#[derive(Clone)]
pub struct StnTheory {
    pub config: StnConfig,
    constraints: ConstraintDb,
    /// Forward/Backward adjacency list containing active edges.
    active_propagators: RefVec<SignedVar, Vec<InlinedPropagator>>,
    incoming_active_propagators: RefVec<SignedVar, Vec<InlinedPropagator>>,
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
    explanation: Vec<PropagatorId>,
    /// When the edge is deactivated due to theory propagation, this field is set to the next event index of the
    /// edge activation trail.
    /// Note that this field is NOT trailed and the value will remain until overriden with a new one.
    /// Hence, the presence of an event index does NOT indicate that the edge is currently deactivated.
    last_disabling_timestamp: RefMap<PropagatorId, EventIndex>,
    pending_bound_changes: Vec<BoundChangeEvent>,
    /// A set of edges whose upper bound is dynamic (i.e. depends on the variable)
    /// The map is indexed on the variable from which the variable is computed.
    dyn_edges: hashbrown::HashMap<SignedVar, Vec<PropagatorId>>,
}

#[derive(Copy, Clone)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum ModelUpdateCause {
    /// The update was caused by an edge propagation
    EdgePropagation(PropagatorId),
    /// Edge propagation detected a cycle, containing this edge
    CyclicEdgePropagation(PropagatorId),
    /// The edge `e` was deactivated because there is an active path p : `e.tgt` -> `e.src`
    /// such that  len(p) + e.weight < 0
    TheoryPropagationPathDeactivation(PropagatorId),
    /// The edge `e` was deactivated because `ub(e.src) + e.weight < lb(e.tgt)`
    TheoryPropagationBoundsDeactivation(PropagatorId),
}

impl From<u32> for ModelUpdateCause {
    fn from(enc: u32) -> Self {
        let determinant = enc & 0b11;
        let payload = enc >> 2;
        match determinant {
            0b00 => ModelUpdateCause::EdgePropagation(PropagatorId::from(payload)),
            0b01 => ModelUpdateCause::CyclicEdgePropagation(PropagatorId::from(payload)),
            0b10 => ModelUpdateCause::TheoryPropagationPathDeactivation(PropagatorId::from(payload)),
            0b11 => ModelUpdateCause::TheoryPropagationBoundsDeactivation(PropagatorId::from(payload)),
            _ => unreachable!(),
        }
    }
}

impl From<ModelUpdateCause> for u32 {
    fn from(cause: ModelUpdateCause) -> Self {
        match cause {
            ModelUpdateCause::EdgePropagation(edge) => u32::from(edge) << 2,
            ModelUpdateCause::CyclicEdgePropagation(edge) => (u32::from(edge) << 2) | 0b01,
            ModelUpdateCause::TheoryPropagationPathDeactivation(edge) => (u32::from(edge) << 2) | 0b10,
            ModelUpdateCause::TheoryPropagationBoundsDeactivation(edge) => (u32::from(edge) << 2) | 0b11,
        }
    }
}

/// Contains the id of a propagator as well as its `target` and `weight` fields that
/// are inlined to facilitate propagation.
#[derive(Copy, Clone, Debug)]
struct InlinedPropagator {
    target: SignedVar,
    weight: IntCst,
    id: PropagatorId,
}

#[derive(Copy, Clone, Debug)]
enum ActivationEvent {
    /// Should activate the given edge, enabled by this literal
    ToEnable {
        /// the edge to enable
        edge: PropagatorId,
        /// The literals that enabled this edge to become active
        enabler: Enabler,
        /// False if theory propagation can be skipped for this edge
        require_theory_propagation: bool,
    },
    /// A dynamic edge requires its weight to be updated
    ToUpdate { edge: PropagatorId },
}

#[derive(Clone, Debug)]
struct BoundChangeEvent {
    var: SignedVar,
    previous_ub: IntCst,
    new_ub: IntCst,
    is_from_bound_propagation: bool,
}

impl StnTheory {
    /// Creates a new STN. Initially, the STN contains a single timepoint
    /// representing the origin whose domain is `[0,0]`. The id of this timepoint can
    /// be retrieved with the `origin()` method.
    pub fn new(config: StnConfig) -> Self {
        StnTheory {
            config,
            constraints: ConstraintDb::new(),
            active_propagators: Default::default(),
            incoming_active_propagators: Default::default(),
            trail: Default::default(),
            pending_activations: VecDeque::new(),
            stats: Default::default(),
            identity: Identity::new(ReasonerId::Diff),
            model_events: ObsTrailCursor::new(),
            explanation: vec![],
            last_disabling_timestamp: Default::default(),
            pending_bound_changes: Default::default(),
            dyn_edges: Default::default(),
        }
    }
    pub fn num_nodes(&self) -> u32 {
        (self.active_propagators.len() / 2) as u32
    }

    pub fn reserve_timepoint(&mut self) {
        // add slots for the propagators of both signed variables
        self.active_propagators.push(Vec::new());
        self.active_propagators.push(Vec::new());
        self.incoming_active_propagators.push(Vec::new());
        self.incoming_active_propagators.push(Vec::new());
    }

    /// Adds a conditional edge `literal => (source ---(weight)--> target)` which is activate when `literal` is true.
    /// The associated propagator will ensure that the domains of the variables are appropriately updated
    /// and that `literal` is set to false if the edge contradicts other constraints.
    // This equivalent to  `literal => (target <= source + weight)`
    pub fn add_half_reified_edge(
        &mut self,
        literal: Lit,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        domains: &Domains,
    ) {
        let source = source.into();
        let target = target.into();
        while u32::from(source) >= self.num_nodes() || u32::from(target) >= self.num_nodes() {
            self.reserve_timepoint();
        }

        // literal that is true if the edge is within its validity scope (i.e. both timepoints are present)
        // edge_valid <=> presence(source) & presence(target)
        let edge_valid = domains.presence(literal.variable());
        debug_assert!(domains.implies(edge_valid, domains.presence(source)));
        debug_assert!(domains.implies(edge_valid, domains.presence(target)));

        // the propagator is valid when `presence(target) => edge_valid`.
        // This is because in this case, the modification to the target's domain are only meaningful if the edge is present.
        // Once the propagator is valid, it can be propagated as soon as its `active` literal becomes true.

        // determine a literal that is true iff a source to target propagator is valid
        let target_propagator_valid = if domains.implies(domains.presence(target), edge_valid) {
            // it is statically known that `presence(target) => edge_valid`,
            // the propagator is always valid
            Lit::TRUE
        } else {
            // given that `presence(source) & presence(target) <=> edge_valid`, we can infer that the propagator becomes valid
            // (i.e. `presence(target) => edge_valid` holds) when `presence(source)` becomes true
            domains.presence(source)
        };
        // determine a literal that is true iff a target to source is valid
        let source_propagator_valid = if domains.implies(domains.presence(source), edge_valid) {
            Lit::TRUE
        } else {
            domains.presence(target)
        };
        let propagators = [
            // normal edge:  active <=> source ---(weight)---> target
            Propagator {
                source: SignedVar::plus(source),
                target: SignedVar::plus(target),
                weight,
                enabler: Enabler::new(literal, target_propagator_valid),
                dyn_weight: None,
            },
            Propagator {
                source: SignedVar::minus(target),
                target: SignedVar::minus(source),
                weight,
                enabler: Enabler::new(literal, source_propagator_valid),
                dyn_weight: None,
            },
        ];

        for p in propagators {
            self.record_propagator(p, domains);
        }
    }

    // Adds a new fully reified edge `literal <=> source ---(weight)---> target`  (STN max delay)
    // This equivalent to  `literal <=> (target <= source + weight)`
    pub fn add_reified_edge(
        &mut self,
        literal: Lit,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        domains: &Domains,
    ) {
        let source = source.into();
        let target = target.into();

        // normal edge:  active <=> source ---(weight)---> target
        self.add_half_reified_edge(literal, source, target, weight, domains);
        // reverse edge:    !active <=> source <----(-weight-1)--- target
        self.add_half_reified_edge(!literal, target, source, -weight - 1, domains);
    }

    /// Add an edge with a dynamic upper bound, representing the fact that `tgt - src <= ub_factor * ub_var`
    pub fn add_dynamic_edge(
        &mut self,
        src: impl Into<Timepoint>,
        tgt: impl Into<Timepoint>,
        ub_var: SignedVar,
        ub_factor: IntCst,
        domains: &Domains,
    ) {
        let source = src.into();
        let target = tgt.into();
        while u32::from(source) >= self.num_nodes() || u32::from(target) >= self.num_nodes() {
            self.reserve_timepoint();
        }
        let edge_valid = domains.presence(ub_var);
        debug_assert!(domains.implies(edge_valid, domains.presence(source)));
        debug_assert!(domains.implies(edge_valid, domains.presence(target)));

        let cur_var_ub = domains.ub(ub_var);
        let cur_ub = cur_var_ub.saturating_mul(ub_factor).min(INT_CST_MAX);
        let literal = ub_var.leq(cur_var_ub);

        // determine a literal that is true iff a source to target propagator is valid
        let target_propagator_valid = if domains.implies(domains.presence(target), edge_valid) {
            // it is statically known that `presence(target) => edge_valid`,
            // the propagator is always valid
            Lit::TRUE
        } else {
            // given that `presence(source) & presence(target) <=> edge_valid`, we can infer that the propagator becomes valid
            // (i.e. `presence(target) => edge_valid` holds) when `presence(source)` becomes true
            domains.presence(source)
        };
        // determine a literal that is true iff a target to source propagator is valid
        let source_propagator_valid = if domains.implies(domains.presence(source), edge_valid) {
            Lit::TRUE
        } else {
            domains.presence(target)
        };

        let propagators = [
            // normal edge:  active <=> source ---(weight)---> target
            Propagator {
                source: SignedVar::plus(source),
                target: SignedVar::plus(target),
                weight: cur_ub,
                enabler: Enabler::new(literal, target_propagator_valid),
                dyn_weight: Some(DynamicWeight {
                    var_ub: ub_var,
                    factor: ub_factor,
                    valid: target_propagator_valid,
                }),
            },
            Propagator {
                source: SignedVar::minus(target),
                target: SignedVar::minus(source),
                weight: cur_ub,
                enabler: Enabler::new(literal, source_propagator_valid),
                dyn_weight: Some(DynamicWeight {
                    var_ub: ub_var,
                    factor: ub_factor,
                    valid: source_propagator_valid,
                }),
            },
        ];

        let propagator_ids = propagators
            .into_iter()
            .map(|p| self.record_propagator(p, domains))
            .collect_vec();

        // record the dynamic edge so that future updates on the variable would trigger a new edge insertion
        let watch_entries = self.dyn_edges.entry(ub_var).or_default();
        for prop_id in propagator_ids {
            watch_entries.push(prop_id);
        }
    }

    /// Creates and record a new propagator associated with the given [DirEdge], making sure
    /// to set up the watches to enable it when it becomes active and valid.
    fn record_propagator(&mut self, prop: Propagator, domains: &Domains) -> PropagatorId {
        let &Enabler { active, valid } = &prop.enabler;
        let edge_valid = domains.presence(active.variable());

        let (prop, new_enabler) = self.constraints.add_propagator(prop);

        match new_enabler {
            PropagatorIntegration::Created(enabler) | PropagatorIntegration::Merged(enabler) => {
                // Add the propagator, with different modalities depending on whether it is currently enabled or not.
                // Note that we should make sure that when backtracking beyond the current decision level, we should deactivate the edge.
                if domains.entails(!active) || domains.entails(!edge_valid) {
                    // do nothing as the propagator can never be active/present
                } else if domains.entails(active) && domains.entails(valid) {
                    // propagator is always active in the current and following decision levels, enqueue it for activation.
                    self.pending_activations.push_back(ActivationEvent::ToEnable {
                        edge: prop,
                        enabler,
                        require_theory_propagation: true,
                    });
                } else {
                    // Not present nor necessarily absent yet, add watches
                    self.constraints.add_propagator_enabler(prop, enabler);
                }
            }
            PropagatorIntegration::Tightened(enabler) => {
                // the propagator set was tightened if already active, we need to force its propagation
                if domains.entails(active) && domains.entails(valid) {
                    // propagator is always active in the current and following decision levels
                    // pretend it was previously inactive (even if it was previously propagated we need to redo it)
                    self.constraints[prop].enabler = None;
                    //enqueue it for activation.
                    self.pending_activations.push_back(ActivationEvent::ToEnable {
                        edge: prop,
                        enabler,
                        require_theory_propagation: true,
                    });
                }
            }
            PropagatorIntegration::Noop => {}
        }
        prop
    }

    fn build_contradiction(&self, culprits: &[PropagatorId], model: &Domains) -> Contradiction {
        let mut expl = Explanation::with_capacity(culprits.len());
        for &edge in culprits {
            debug_assert!(self.active(edge));
            let enabler = self.constraints[edge].enabler;
            let enabler = enabler.expect("No established enabler for this edge").0;
            debug_assert!(model.entails(enabler.active) && model.entails(enabler.valid));
            expl.push(enabler.active);
            expl.push(enabler.valid);
        }
        Contradiction::Explanation(expl)
    }

    fn explain_bound_propagation(
        &self,
        event: Lit,
        propagator: PropagatorId,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        debug_assert!(self.active(propagator));
        let c = &self.constraints[propagator];
        let val = event.ub_value();
        debug_assert_eq!(event.svar(), c.target);

        // add literal to explanation (in debug, checks that the literal is indeed entailed)
        let mut add_to_expl = |l: Lit| {
            debug_assert!(model.entails(l));
            out_explanation.push(l);
        };

        if let Some(dyn_weight) = c.dyn_weight {
            // The edge is dynamic, hence the weight on the propagator is not necessarily the one it had
            // when the propagation was triggered.
            // We need to recompute the weight it had (or a stronger it could have had).
            let var_ub = model.ub(dyn_weight.var_ub);
            let weight = var_ub * dyn_weight.factor;
            add_to_expl(dyn_weight.valid);
            add_to_expl(dyn_weight.var_ub.leq(var_ub));
            add_to_expl(c.source.leq(val - weight));
        } else {
            let enabler = c.enabler.expect("inactive constraint").0;
            add_to_expl(enabler.active);
            add_to_expl(enabler.valid);

            let cause = c.source.leq(val - c.weight);
            add_to_expl(cause);
        }
    }

    /// Propagates all edges that have been marked as active since the last propagation.
    pub fn propagate_all(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        // in first propagation, process each edge once to check if it can be added to the model based on the literals
        // of its extremities. If it is not the case, make its enablers false.
        // This step is equivalent to "bound theory propagation" but need to be made independently because
        // we do not get events for the initial domain of the variables.
        if self.config.theory_propagation.bounds() {
            while let Some(c_id) = self.constraints.next_new_constraint() {
                let c = &self.constraints[c_id];
                // ignore enabled edges, they are dealt with by normal propagation
                if c.enabler.is_none() {
                    // new upper bound of target that would be derived if we were to add this edge
                    let new_ub = model.ub(c.source) + c.weight;
                    let current_lb = model.lb(c.target);
                    if new_ub < current_lb || c.source == c.target && c.weight < 0 {
                        // the edge is invalid, build a cause to allow explanation
                        let cause = self
                            .identity
                            .inference(ModelUpdateCause::TheoryPropagationBoundsDeactivation(c_id));
                        // make all enablers false
                        for &l in &c.enablers {
                            let change = model.set(!l.active, cause)?;
                            if change {
                                self.stats.num_bound_edge_deactivation += 1;
                            }
                        }
                    }
                }
            }
        }

        while self.model_events.num_pending(model.trail()) > 0 || !self.pending_activations.is_empty() {
            // start by propagating all literals changes before considering the new edges.
            // This is necessary because cycle detection on the insertion of a new edge requires
            // a consistent STN and no interference of external bound updates.
            while let Some(ev) = self.model_events.pop(model.trail()).copied() {
                // describes the origin of the event
                #[derive(PartialEq)]
                enum Origin {
                    // not emitted by us
                    External,
                    // originates from bound propagation
                    BoundPropagation,
                    // detection of a cycle during bound propagation, which would infer the absence of some timepoint
                    CycleDetection,
                    // originates from theory propagation
                    TheoryPropagation,
                }
                let origin = if let Some(x) = ev.cause.as_external_inference() {
                    if x.writer != self.identity.writer_id {
                        Origin::External
                    } else {
                        // we emitted this event, we can safely convert the payload
                        match ModelUpdateCause::from(x.payload) {
                            ModelUpdateCause::EdgePropagation(_) => Origin::BoundPropagation,
                            ModelUpdateCause::CyclicEdgePropagation(_) => Origin::CycleDetection,
                            ModelUpdateCause::TheoryPropagationPathDeactivation(_) => Origin::TheoryPropagation,
                            ModelUpdateCause::TheoryPropagationBoundsDeactivation(_) => Origin::TheoryPropagation,
                        }
                    }
                } else {
                    Origin::External
                };
                let literal = ev.new_literal();
                for (enabler, edge) in self.constraints.enabled_by(literal) {
                    // mark active
                    if model.entails(enabler.active) && model.entails(enabler.valid) {
                        self.pending_activations.push_back(ActivationEvent::ToEnable {
                            edge,
                            enabler,
                            // if the edge was enabled through theory propagation, then it does not need to be propagated
                            // as there is already a path that is at least as strong
                            require_theory_propagation: origin != Origin::TheoryPropagation,
                        });
                    }
                }

                // go through all dynamic edges whose upper bound is given by this variable.
                if let Some(dyn_edges_on_bound) = self.dyn_edges.get(&ev.affected_bound) {
                    for &prop_id in dyn_edges_on_bound {
                        self.pending_activations
                            .push_back(ActivationEvent::ToUpdate { edge: prop_id });
                    }
                }
                if self.constraints.is_vertex(ev.affected_bound) {
                    // ignore events from bound propagation as they would be already propagated
                    self.pending_bound_changes.push(BoundChangeEvent {
                        var: ev.affected_bound,
                        previous_ub: ev.previous.upper_bound,
                        new_ub: ev.new_upper_bound,
                        is_from_bound_propagation: origin == Origin::BoundPropagation,
                    });
                }
            }
            // run dijkstra from all updates, without cycle detection (we know there are none)
            bound_propagation::process_bound_changes(self, model, |_| false)?;

            // very costly check, deactivated by default
            // #[cfg(debug_assertions)]
            // self.assert_fully_bound_propagated(model);

            while let Some(event) = self.pending_activations.pop_front() {
                // If get there all bounds should be propagated, meaning that the potential function should be valid
                // The check is deactivated here because it is a very expensive check (even when debugging)
                // debug_assert!(StnGraph::new(self, model).is_potential_valid());

                let new_edge_to_propagate = match event {
                    ActivationEvent::ToEnable {
                        edge,
                        enabler,
                        require_theory_propagation,
                    } => {
                        let c = &mut self.constraints[edge];
                        if c.enabler.is_none() {
                            // edge is currently inactive
                            if c.source == c.target {
                                // we are in a self loop, that must handled separately since they are trivial
                                // to handle and not supported by the propagation loop
                                if c.weight < 0 {
                                    // negative self loop: inconsistency
                                    self.explanation.clear();
                                    self.explanation.push(edge); // TODO: may not be an error, instead we shoud make the node absent?
                                    return Err(self.build_contradiction(&self.explanation, model));
                                } else {
                                    // positive self loop : useless edge that we can ignore
                                    None
                                }
                            } else {
                                debug_assert_ne!(c.source, c.target);
                                let activation_event = self.trail.push(EdgeActivated(edge));
                                c.enabler = Some((enabler, activation_event));
                                // add the edge to the active propagator lists, recording their index (which is used for updating dynamic edges)
                                let c = &mut self.constraints[edge];
                                c.index_in_active = self.active_propagators[c.source].len() as u32;
                                self.active_propagators[c.source].push(InlinedPropagator {
                                    target: c.target,
                                    weight: c.weight,
                                    id: edge,
                                });
                                c.index_in_incoming_active = self.incoming_active_propagators[c.target].len() as u32;
                                self.incoming_active_propagators[c.target].push(InlinedPropagator {
                                    target: c.source,
                                    weight: c.weight,
                                    id: edge,
                                });

                                // check if the edge is obviously redundant, i.e., the bounds are sufficient to entail it
                                // If that is the case, there is no need to propagate it at all since all inference could have been made based on the bounds only
                                let redundant = -model.lb(c.source) + model.ub(c.target) <= c.weight;
                                if redundant {
                                    None
                                } else {
                                    // notify that this edge must now be propagated
                                    Some((edge, require_theory_propagation))
                                }
                            }
                        } else {
                            None
                        }
                    }
                    ActivationEvent::ToUpdate { edge } => {
                        let prop = &mut self.constraints[edge];
                        let dyn_weight = prop.dyn_weight.unwrap();
                        let previous_weight = prop.weight;
                        let previous_enabler = prop.enabler;
                        let new_weight =
                            (model.ub(dyn_weight.var_ub) * dyn_weight.factor).clamp(INT_CST_MIN, INT_CST_MAX);

                        // check if an update is needed
                        // it might be the case that no update are required if the variable's upper bound was updated multiple times since the last propagation
                        if new_weight < previous_weight {
                            let activation_event = self.trail.push(Event::EdgeUpdated {
                                prop: edge,
                                previous_weight,
                                previous_enabler,
                            });
                            prop.weight = new_weight;

                            // if the edge was previously enabled, update it
                            if previous_enabler.is_some() {
                                let activation = dyn_weight.var_ub.leq(model.ub(dyn_weight.var_ub));
                                prop.enabler = Some((
                                    Enabler {
                                        active: activation,
                                        valid: dyn_weight.valid,
                                    },
                                    activation_event,
                                ));
                                // the edge was active, we must update the weights in the inlined propagators
                                // the edge is enabled, update the weight of the inlined propagators (both forward and backward)
                                let inlined = &mut self.active_propagators[prop.source][prop.index_in_active as usize];
                                debug_assert_eq!(inlined.id, edge);
                                debug_assert_eq!(inlined.weight, previous_weight);
                                inlined.weight = new_weight;

                                let inlined = &mut self.incoming_active_propagators[prop.target]
                                    [prop.index_in_incoming_active as usize];
                                debug_assert_eq!(inlined.id, edge);
                                debug_assert_eq!(inlined.weight, previous_weight);
                                inlined.weight = new_weight;
                            } else {
                                // TODO: we could here update the weight of the intermittent propagator but it is a use case that is very unlikely to occur in practice
                                // for the problems we consider
                            }
                            Some((edge, true))
                        } else {
                            None
                        }
                    }
                };
                if let Some((edge, require_theory_propagation)) = new_edge_to_propagate {
                    // propagate bounds from this edge
                    // As a consequence, it re-establishes the validity of our potential function
                    self.propagate_new_edge(edge, model)?;

                    // after propagating the new edge, the potential function should be valid
                    // The check is deactivated here because it is very expensive (even when debugging)
                    // debug_assert!(StnGraph::new(self, model).is_potential_valid());

                    if self.config.theory_propagation.edges() && require_theory_propagation {
                        self.theory_propagate_edge(edge, model)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// (Very) expensive cehck that the STN is full propagated with respect to the bounds.
    /// It verifies that for each edge its propagation would be a no-op if it is already enabled
    /// and that for each pending edge, it has been properly deactivated/activated if its implied by the bounds
    #[cfg(debug_assertions)]
    #[allow(unused)]
    fn assert_fully_bound_propagated(&self, doms: &Domains) {
        for (_id, prop) in self.constraints.propagators() {
            if doms.present(prop.source) == Some(false) || doms.present(prop.target) == Some(false) {
                continue;
            }
            if prop.enabler.is_some() {
                assert!(doms.ub(prop.target) <= doms.ub(prop.source) + prop.weight);
            } else {
                // this edge is not enabled
                // the path source -> orig -> target  should be weaker that the edge
                if -doms.lb(prop.source) + doms.ub(prop.target) <= prop.weight {
                    for e in &prop.enablers {
                        assert!(doms.entails(e.active) || doms.present(e.active) == Some(false));
                    }
                }

                if -doms.lb(prop.target) + doms.ub(prop.source) + prop.weight < 0 {
                    for e in &prop.enablers {
                        assert!(
                            doms.entails(!e.active) || doms.entails(!e.valid) || doms.present(e.active) == Some(false)
                        );
                    }
                }
            }
        }
    }

    /// Creates a new backtrack point that represents the STN at the point of the method call,
    /// just before the insertion of the backtrack point.
    pub fn set_backtrack_point(&mut self) -> BacktrackLevel {
        assert!(
            self.pending_activations.is_empty(),
            "Cannot set a backtrack point if a propagation is pending. \
            The code introduced in this commit should enable this but has not been thoroughly tested yet."
        );
        self.trail.save_state();
        self.constraints.save_state()
    }

    pub fn undo_to_last_backtrack_point(&mut self) -> Option<BacktrackLevel> {
        // remove pending activations
        // invariant: there are no pending activation when saving the state
        self.pending_activations.clear();

        // undo changes since the last backtrack point
        self.trail.restore_last_with(|ev| match ev {
            EdgeActivated(e) => {
                let c = &mut self.constraints[e];
                self.active_propagators[c.source].pop();
                self.incoming_active_propagators[c.target].pop();
                c.enabler = None;
            }
            Event::EdgeUpdated {
                prop,
                previous_weight,
                previous_enabler,
            } => {
                let c = &mut self.constraints[prop];
                c.weight = previous_weight;
                c.enabler = previous_enabler;
                if previous_enabler.is_some() {
                    // the edge had inlined propagators, restore their weight
                    let inlined = &mut self.active_propagators[c.source][c.index_in_active as usize];
                    debug_assert_eq!(inlined.id, prop);
                    inlined.weight = previous_weight;
                    let inlined = &mut self.incoming_active_propagators[c.target][c.index_in_incoming_active as usize];
                    debug_assert_eq!(inlined.id, prop);
                    inlined.weight = previous_weight;
                }
            }
        });
        self.constraints.restore_last();

        None
    }

    fn active(&self, e: PropagatorId) -> bool {
        self.constraints[e].enabler.is_some()
    }

    /// Implementation of [Cesta96]
    /// It propagates a **newly_inserted** edge in a **consistent** STN.
    fn propagate_new_edge(&mut self, new_edge: PropagatorId, model: &mut Domains) -> Result<(), Contradiction> {
        let c = &self.constraints[new_edge];
        debug_assert_ne!(c.source, c.target, "This algorithm does not support self loops.");
        let cause = self.identity.inference(ModelUpdateCause::EdgePropagation(new_edge));
        let source = c.source;
        let target = c.target;
        let weight = c.weight;
        let source_bound = model.ub(source);
        let prev = model.ub(target);
        let new = source_bound + weight;
        if model.set_upper_bound(target, source_bound + weight, cause)? {
            // set up the updates to be considered for bound propagation
            debug_assert!(self.pending_bound_changes.is_empty());
            self.pending_bound_changes.push(BoundChangeEvent {
                var: target,
                previous_ub: prev,
                new_ub: new,
                is_from_bound_propagation: false,
            });
            // run propagation from target, indicating that if the propagation cycles back to source,
            // it indicates that there is a negative cycle containing the new edge
            bound_propagation::process_bound_changes(self, model, |v| v == source)?;
        }

        Ok(())
    }

    /// Extract a negative cycle detected during bound propagation, where the last edge is given as input.
    /// It is expected that there is a chain of bound updates from `last_edge.target` until ``last_edge.source`
    ///
    /// The explanation would be the activation literals of all edges in the cycle.
    fn extract_cycle(&self, propagator_id: PropagatorId, model: &DomainsSnapshot, expl: &mut Explanation) {
        let last_edge_of_cycle = &self.constraints[propagator_id];
        let last_edge_trigger = last_edge_of_cycle.enabler.expect("inactive edge").0;
        debug_assert!(model.entails(last_edge_trigger.active));
        debug_assert!(model.entails(last_edge_trigger.valid));
        // add this edge to the explanation
        expl.push(last_edge_trigger.active);
        expl.push(last_edge_trigger.valid);

        let mut curr = last_edge_of_cycle.source;
        let mut cycle_length = last_edge_of_cycle.weight;

        // now go back from src until we find the target node, adding all edges on the path
        loop {
            let ub = model.ub(curr);
            let lit = Lit::leq(curr, ub);
            debug_assert!(model.entails(lit));
            let ev = model.implying_event(lit).unwrap();
            debug_assert_eq!(model.entailing_level(lit), self.trail.current_decision_level());
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
            cycle_length += c.weight;
            let trigger = self.constraints[edge].enabler.expect("inactive constraint").0;
            debug_assert!(model.entails(trigger.active));
            debug_assert!(model.entails(trigger.valid));
            // add edge to the explanation
            expl.push(trigger.active);
            expl.push(trigger.valid);

            if curr == last_edge_of_cycle.target {
                // we have completed the cycle
                debug_assert!(cycle_length < 0, "cycle length: {cycle_length}");
                break;
            }
        }
    }

    pub fn print_stats(&self) {
        println!("# nodes: {}", self.num_nodes());
        println!("# propagators: {}", self.constraints.num_propagator_groups());
        println!("# propagations: {}", self.stats.num_propagations);
        println!("# domain updates: {}", self.stats.bound_updates);
        println!("# bounds deactivations: {}", self.stats.num_bound_edge_deactivation);
        println!("# theory propagations: {}", self.stats.num_theory_propagations);
        println!("# theory deactivations: {}", self.stats.num_theory_deactivations);
    }

    /// Perform the theory propagation that follows from the addition of the given edge.
    ///
    /// In essence, we find all shortest paths A -> B that contain the new edge.
    /// Then we check if there exist an inactive edge BA where `weight(BA) + dist(AB) < 0`.
    /// For each such edge, we set its enabler to false since its addition would result in a negative cycle.
    #[inline(never)]
    fn theory_propagate_edge(&mut self, edge: PropagatorId, model: &mut Domains) -> Result<(), Contradiction> {
        // get reusable memory from thread-local storage
        distances::STATE.with_borrow_mut(|(heap, pot_updates)| {
            let constraint = &self.constraints[edge];
            let target = constraint.target;
            let source = constraint.source;
            let weight = constraint.weight;

            if model.present(target) == Some(false) {
                return Ok(());
            }
            self.stats.num_theory_propagations += 1;

            // view of the STN as graph, excluding the additional edge
            // the additional was previously inserted to enable explanations
            let stn = StnGraph::new_excluding(self, model, edge);

            stn.updated_on_addition_no_alloc(source, target, weight, edge, pot_updates, heap);

            for &(dest, dist_to_dest) in &pot_updates.postfixes {
                // TODO: the loop below is a source of inefficiency, we already know which edges could be checked
                for potential in self.constraints.potential_out_edges(dest) {
                    let orig = potential.target;
                    if let Some(dist_from_orig) = pot_updates.get_prefix(orig) {
                        let new_path_length = dist_from_orig + weight + dist_to_dest;
                        if new_path_length + potential.weight < 0 {
                            // edge should be deactivated
                            // update the model to force this edge to be inactive

                            // before changing anything in the model compute the path that we would get in an explanation
                            // deactivated below because it is to expensive, even in debug mode
                            // let full_stn = StnGraph::new(self, model);
                            // let ssp = full_stn.shortest_distance(orig, dest);

                            let res = model.set(
                                !potential.presence,
                                self.identity
                                    .inference(ModelUpdateCause::TheoryPropagationPathDeactivation(potential.id)),
                            );
                            if res != Ok(false) {
                                self.stats.num_theory_deactivations += 1;
                                // something was changed, either a domain update or an error
                                // we thus must be able to explain it
                                // Checks that there is indeed a shortest path just before the update (deactivated for performance reason)
                                // debug_assert!({ ssp.expect("no path") <= new_path_length })

                                // set the deactivation timestamp so that we can consider the graph as it was when we made the inference
                                self.last_disabling_timestamp
                                    .insert(potential.id, self.trail.next_event());
                                // rethrow the error if any
                                res?;
                            }
                        }
                    }
                }
            }
            Ok(())
        })
    }
}

impl Theory for StnTheory {
    fn identity(&self) -> ReasonerId {
        self.identity.writer_id
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        self.propagate_all(model)
    }

    fn explain(
        &mut self,
        event: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        debug_assert_eq!(context.writer, self.identity());
        let context = context.payload;
        match ModelUpdateCause::from(context) {
            ModelUpdateCause::EdgePropagation(edge_id) => {
                self.explain_bound_propagation(event, edge_id, model, out_explanation)
            }
            ModelUpdateCause::CyclicEdgePropagation(edge_id) => self.extract_cycle(edge_id, model, out_explanation),
            ModelUpdateCause::TheoryPropagationBoundsDeactivation(edge_id) => {
                // TODO: we can be a strong explanation here
                let edge = &self.constraints[edge_id];
                // having this edge would have entailed `edge.tgt >= ub(edge.src) + edge.weight
                // which we have determined to be in contradiction with the current lower bound of edge.tgt
                let src_ub = model.ub(edge.source);
                let tgt_lb = model.lb(edge.target);
                debug_assert!(src_ub + edge.weight < tgt_lb);
                out_explanation.push(edge.source.leq(src_ub));
                out_explanation.push(edge.target.geq(tgt_lb));
            }
            ModelUpdateCause::TheoryPropagationPathDeactivation(edge_id) => {
                // edge that was deactivated
                let edge = &self.constraints[edge_id];
                // size of the trail at the moment of the deactivation
                let event_after = self.last_disabling_timestamp[edge_id];
                // construct a view of the graph at the time of the deactivation
                let graph = distances::StnSnapshotGraph::new(self, model, event_after);
                let path = graph
                    .shortest_path(edge.target, edge.source)
                    .expect("No explaining path in graph");
                let mut path_length = 0;
                for edge_path_id in path {
                    let edge_path = &self.constraints[edge_path_id];
                    path_length += edge_path.weight;
                    let (enabler, activation) = edge_path.enabler.expect("Inactive edge on path");
                    out_explanation.push(enabler.active);
                    out_explanation.push(enabler.valid); // TODO: since we are only talking about edges, are we allowed to omit this in the explanations?
                    debug_assert!(activation < event_after);
                    debug_assert!(model.entails(enabler.active));
                    debug_assert!(model.entails(enabler.valid));
                }
                debug_assert!(path_length + edge.weight < 0);
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

#[allow(clippy::let_unit_value)]
#[cfg(test)]
mod tests {
    use crate::model::extensions::AssignmentExt;
    use crate::model::lang::IVar;

    use crate::reasoners::stn::stn_impl::Stn;

    use super::*;

    #[test]
    fn test_propagation() {
        let s = &mut Stn::new();
        let a = s.add_timepoint(0, 10);
        let b = s.add_timepoint(0, 10);

        let assert_bounds = |stn: &Stn, a_lb, a_ub, b_lb, b_ub| {
            assert_eq!(stn.model.int_bounds(IVar::new(a)), (a_lb, a_ub));
            assert_eq!(stn.model.int_bounds(IVar::new(b)), (b_lb, b_ub));
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
            assert_eq!(stn.model.int_bounds(IVar::new(a)), (a_lb, a_ub));
            assert_eq!(stn.model.int_bounds(IVar::new(b)), (b_lb, b_ub));
        };

        assert_bounds(s, 0, 10, 0, 10);

        s.set_ub(a, 1);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 10);
        s.set_backtrack_point();

        let ab = s.add_edge(a, b, 5);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 6);

        s.set_backtrack_point();

        let ba = s.add_edge(b, a, -6);
        s.assert_inconsistent(vec![ab, ba]);

        s.undo_to_last_backtrack_point();
        assert_bounds(s, 0, 1, 0, 6);

        s.undo_to_last_backtrack_point();
        assert_bounds(s, 0, 1, 0, 10);

        let x = s.add_inactive_edge(a, b, 5);
        s.mark_active(x);
        s.assert_consistent();
        assert_bounds(s, 0, 1, 0, 6);
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

        stn.add_delay(a, b, 0);

        stn.propagate_all()?;
        stn.model.state.set_lb(b, 1, Cause::Decision)?;
        stn.model.state.set_ub(b, 9, Cause::Decision)?;

        stn.propagate_all()?;
        assert_eq!(stn.model.domain_of(a), (0, 10));
        assert_eq!(stn.model.domain_of(b), (1, 9));

        stn.model.state.set_lb(a, 2, Cause::Decision)?;

        stn.propagate_all()?;
        assert_eq!(stn.model.domain_of(a), (2, 10));
        assert_eq!(stn.model.domain_of(b), (2, 9));

        stn.model.state.set(prez_b, Cause::Decision)?;

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
                stn.add_delay(vars[i - 1].1, var, 1);
            }
            vars.push((prez, var));
            context = prez;
        }

        stn.propagate_all()?;
        for (i, (_prez, var)) in vars.iter().enumerate() {
            let i: IntCst = i.try_into().unwrap();
            assert_eq!(stn.model.int_bounds(*var), (i, 20));
        }
        stn.model.state.set_ub(vars[5].1, 4, Cause::Decision)?;
        stn.propagate_all()?;
        for (i, (_prez, var)) in vars.iter().enumerate() {
            let i: IntCst = i.try_into().unwrap();
            if i <= 4 {
                assert_eq!(stn.model.int_bounds(*var), (i, 20));
            } else {
                assert_eq!(stn.model.state.present(*var), Some(false))
            }
        }

        Ok(())
    }

    #[test]
    fn test_theory_propagation_edges_simple() -> Result<(), Contradiction> {
        let stn = &mut Stn::new_with_config(StnConfig {
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
        // let bottom = stn.add_inactive_edge(a1, b1, -1);

        stn.propagate_all()?;
        assert_eq!(stn.model.state.bounds(a1), (10, 20));
        assert_eq!(stn.model.state.bounds(b1), (10, 20));
        stn.model.state.set(top, Cause::Decision)?;
        stn.propagate_all()?;

        // TODO: optional propagation currently does not takes an edge whose source is not proved present
        // assert!(stn.model.entails(!bottom));

        Ok(())
    }

    // #[test]
    // fn test_distances() -> Result<(), Contradiction> {
    //     let stn = &mut Stn::new();

    //     // create an STN graph with the following edges, all with a weight of 1
    //     // A ---> C ---> D ---> E ---> F
    //     // |                    ^
    //     // --------- B ----------
    //     let a = stn.add_timepoint(0, 10);
    //     let b = stn.add_timepoint(0, 10);
    //     let c = stn.add_timepoint(0, 10);
    //     let d = stn.add_timepoint(0, 10);
    //     let e = stn.add_timepoint(0, 10);
    //     let f = stn.add_timepoint(0, 10);
    //     stn.add_edge(a, b, 1);
    //     stn.add_edge(a, c, 1);
    //     stn.add_edge(c, d, 1);
    //     stn.add_edge(b, e, 1);
    //     stn.add_edge(d, e, 1);
    //     stn.add_edge(e, f, 1);

    //     stn.propagate_all()?;

    //     let dists = stn.stn.forward_dist(a, &stn.model.state);
    //     assert_eq!(dists.entries().count(), 6);
    //     assert_eq!(dists[a], 0);
    //     assert_eq!(dists[b], 1);
    //     assert_eq!(dists[c], 1);
    //     assert_eq!(dists[d], 2);
    //     assert_eq!(dists[e], 2);
    //     assert_eq!(dists[f], 3);

    //     let dists = stn.stn.backward_dist(a, &stn.model.state);
    //     assert_eq!(dists.entries().count(), 1);
    //     assert_eq!(dists[a], 0);

    //     let dists = stn.stn.backward_dist(f, &stn.model.state);
    //     assert_eq!(dists.entries().count(), 6);
    //     assert_eq!(dists[f], 0);
    //     assert_eq!(dists[e], -1);
    //     assert_eq!(dists[d], -2);
    //     assert_eq!(dists[b], -2);
    //     assert_eq!(dists[c], -3);
    //     assert_eq!(dists[a], -3);

    //     let dists = stn.stn.backward_dist(d, &stn.model.state);
    //     assert_eq!(dists.entries().count(), 3);
    //     assert_eq!(dists[d], 0);
    //     assert_eq!(dists[c], -1);
    //     assert_eq!(dists[a], -2);

    //     Ok(())
    // }

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

    // #[test]
    // fn test_distances_simple() -> Result<(), Contradiction> {
    //     let stn = &mut Stn::new();

    //     // create an STN graph with the following edges, all with a weight of 1
    //     // A ---> C ---> D ---> E ---> F
    //     // |                    ^
    //     // --------- B ----------
    //     let a = stn.add_timepoint(0, 1);
    //     let b = stn.add_timepoint(0, 10);
    //     stn.add_edge(b, a, -1);
    //     stn.propagate_all()?;

    //     let dists = stn.stn.backward_dist(a, &stn.model.state);
    //     assert_eq!(dists.entries().count(), 2);
    //     assert_eq!(dists[a], 0);
    //     assert_eq!(dists[b], 1);

    //     Ok(())
    // }

    #[test]
    fn test_theory_propagation_edges() -> Result<(), Contradiction> {
        let stn = &mut Stn::new_with_config(StnConfig {
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

        assert_eq!(stn.model.state.value(ba0), None);
        stn.propagate_all()?;
        assert_eq!(stn.model.state.value(ba0), None);
        assert_eq!(stn.model.state.value(ba1), None);
        assert_eq!(stn.model.state.value(ba2), Some(false));

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
        assert_eq!(stn.model.state.value(ga0), None);
        assert_eq!(stn.model.state.value(ga1), None);
        assert_eq!(stn.model.state.value(ga2), Some(false));

        let exp = stn.explain_literal(!ga2);
        assert_eq!(exp.len(), 1);

        Ok(())
    }

    #[test]
    fn test_theory_propagation_bounds() -> Result<(), Contradiction> {
        let stn = &mut Stn::new_with_config(StnConfig {
            theory_propagation: TheoryPropagationLevel::Bounds,
            ..Default::default()
        });

        let a = stn.add_timepoint(0, 10);
        let b = stn.add_timepoint(10, 20);

        // inactive edge stating that  b <= a
        let edge_trigger = stn.add_inactive_edge(a, b, 0);
        stn.propagate_all()?;
        assert_eq!(stn.model.state.value(edge_trigger), None);

        stn.set_backtrack_point();
        stn.model.state.set_lb(b, 11, Cause::Decision)?;
        stn.propagate_all()?; // HERE
        assert_eq!(stn.model.state.value(edge_trigger), Some(false));

        stn.undo_to_last_backtrack_point();
        stn.set_backtrack_point();
        stn.model.state.set_ub(a, 9, Cause::Decision)?;
        stn.propagate_all()?;
        assert_eq!(stn.model.state.value(edge_trigger), Some(false));

        Ok(())
    }

    #[test]
    fn test_dynamic_edges() -> Result<(), Contradiction> {
        let max = 100;
        let stn = &mut Stn::new();
        let a = stn.add_timepoint(0, 0);
        let b = stn.add_timepoint(0, 1000000);

        let ub = stn.add_timepoint(0, max);
        stn.add_dynamic_edge(a, b, SignedVar::plus(ub), 2);

        stn.propagate_all()?;
        stn.stn.print_stats();
        let print = |stn: &Stn| {
            println!("a:  {:?} {a:?}", stn.model.domain_of(a));
            println!("b:  {:?} {b:?}", stn.model.domain_of(b));
            println!("ub: {:?} {ub:?}", stn.model.domain_of(ub));
            println!("dl: {:?}", stn.model.current_decision_level());
        };

        print(stn);

        // given the single literal (expected to be an upperbound of `ub`) that caused a the update of the upper bound of `b`
        let cause_b_ub = |stn: &mut Stn, b_ub: IntCst| {
            let implying = stn.implying_literals(Lit::leq(b, b_ub)).unwrap();
            assert_eq!(implying.len(), 1);
            implying[0]
        };

        for i in 5..=max {
            stn.set_backtrack_point();
            stn.model.state.set_ub(ub, max - i, Cause::Decision)?;
            stn.propagate_all()?;
            print(stn);
            stn.stn.print_stats();
            let b_ub = (max - i) * 2;
            // check that propagation is correct
            debug_assert_eq!(stn.model.domain_of(b), (0, b_ub));
            //print(stn);

            assert_eq!(cause_b_ub(stn, b_ub), Lit::leq(ub, max - i));
            assert_eq!(cause_b_ub(stn, b_ub + 1), Lit::leq(ub, max - i));
            if (max - i) < 50 {
                assert_eq!(cause_b_ub(stn, 101), Lit::leq(ub, 50));
                assert_eq!(cause_b_ub(stn, 100), Lit::leq(ub, 50));
                assert_eq!(cause_b_ub(stn, 99), Lit::leq(ub, 49));
            }
        }
        stn.stn.print_stats();
        print(stn);

        Ok(())
    }
}
