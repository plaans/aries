use crate::solver::search::{Decision, SearchControl};
use crate::solver::stats::Stats;
use aries_backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail};
use aries_collections::heap::IdxHeap;
use aries_collections::ref_store::RefMap;
use aries_model::extensions::{AssignmentExt, SavedAssignment, Shaped};
use aries_model::lang::{IntCst, VarRef};
use aries_model::literals::{Lit, Watches};
use aries_model::state::{Event, IntDomain};
use aries_model::{Label, Model};
use env_param::EnvParam;
use itertools::Itertools;
use std::sync::Arc;

pub static PREFER_MIN_VALUE: EnvParam<bool> = EnvParam::new("ARIES_SMT_PREFER_MIN_VALUE", "true");
pub static INITIALLY_ALLOWED_CONFLICTS: EnvParam<u64> = EnvParam::new("ARIES_SMT_INITIALLY_ALLOWED_CONFLICT", "100");
pub static INCREASE_RATIO_FOR_ALLOWED_CONFLICTS: EnvParam<f32> =
    EnvParam::new("ARIES_SMT_INCREASE_RATIO_FOR_ALLOWED_CONFLICTS", "1.5");
pub static USE_LNS: EnvParam<bool> = EnvParam::new("ARIES_ACTIVITY_USES_LNS", "true");

#[derive(Clone)]
pub struct BranchingParams {
    pub prefer_min_value: bool,
    pub allowed_conflicts: u64,
    pub increase_ratio_for_allowed_conflicts: f32,
}

impl Default for BranchingParams {
    fn default() -> Self {
        BranchingParams {
            prefer_min_value: PREFER_MIN_VALUE.get(),
            allowed_conflicts: INITIALLY_ALLOWED_CONFLICTS.get(),
            increase_ratio_for_allowed_conflicts: INCREASE_RATIO_FOR_ALLOWED_CONFLICTS.get(),
        }
    }
}

pub trait Heuristic<Lbl>: Send + Sync + 'static {
    /// Specifies at which decision stage the given variable should be handled.
    ///
    /// The brancher only starts branching on the variables of stage N when
    /// all variables of stage (N - 1) are set.
    fn decision_stage(&self, var: VarRef, label: Option<&Lbl>, model: &Model<Lbl>) -> u8;
}

/// Default branching heuristic that puts all variables in the same decision stage.
pub struct DefaultHeuristic;

impl<L> Heuristic<L> for DefaultHeuristic {
    fn decision_stage(&self, _: VarRef, _: Option<&L>, _: &Model<L>) -> u8 {
        0
    }
}

/// A branching scheme that first select variables that were recently involved in conflicts.
#[derive(Clone)]
pub struct ActivityBrancher<Lbl> {
    pub params: BranchingParams,
    heuristic: Arc<dyn Heuristic<Lbl>>,
    heap: VarSelect,
    default_assignment: DefaultValues,
    conflicts_at_last_restart: u64,
    num_processed_var: usize,
    /// Associates presence literals to the optional variables
    /// Essentially a Map<Lit, Set<VarRef>>
    presences: Watches<VarRef>,
    cursor: ObsTrailCursor<Event>,
}

#[derive(Clone, Default)]
struct DefaultValues {
    /// If these default values came from a valid assignment, this is the value of the associated objective
    objective_found: Option<IntCst>,
    /// Default value for variables (some variables might not have one)
    values: RefMap<VarRef, IntCst>,
}

impl<Lbl: Label> ActivityBrancher<Lbl> {
    pub fn new() -> Self {
        Self::new_with(Default::default(), DefaultHeuristic)
    }

    pub fn new_with_heuristic(h: impl Heuristic<Lbl>) -> Self {
        Self::new_with(BranchingParams::default(), h)
    }

    pub fn new_with_params(params: BranchingParams) -> Self {
        Self::new_with(params, DefaultHeuristic)
    }

    pub fn new_with(params: BranchingParams, h: impl Heuristic<Lbl>) -> Self {
        ActivityBrancher {
            params,
            heuristic: Arc::new(h),
            heap: VarSelect::new(Default::default()),
            default_assignment: DefaultValues::default(),
            conflicts_at_last_restart: 0,
            num_processed_var: 0,
            presences: Default::default(),
            cursor: ObsTrailCursor::new(),
        }
    }

    fn priority(&self, variable: VarRef, model: &Model<Lbl>) -> u8 {
        self.heuristic
            .decision_stage(variable, model.get_label(variable), model)
    }

    pub fn import_vars(&mut self, model: &Model<Lbl>) {
        let mut count = 0;
        // go through the model's variables and declare any newly declared variable
        // TODO: use `advance_by` when it is stabilized. The current usage of `dropping` is very expensive
        //       when compiled without opt-level=3. advance_by should be easier to optimize but dropping will
        //       have to wait to adopt it. [Tracking issue](https://github.com/rust-lang/rust/issues/77404)
        for var in model.state.variables().dropping(self.num_processed_var) {
            debug_assert!(!self.heap.is_declared(var));
            let prez = model.presence_literal(var);
            self.heap.declare_variable(var, self.priority(var, model), None);
            // remember that, when `prez` becomes true we must enqueue the variable
            self.presences.add_watch(var, prez);

            // `prez` is already true, enqueue the variable immediately
            if model.entails(prez) {
                self.heap.enqueue_variable(var);
            }
            count += 1;
        }
        self.num_processed_var += count;

        // process all new events and enqueue the variables that became present
        while let Some(x) = self.cursor.pop(model.state.trail()) {
            for var in self.presences.watches_on(x.new_literal()) {
                self.heap.enqueue_variable(var);
            }
        }
    }

    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    pub fn next_decision(&mut self, stats: &Stats, model: &Model<Lbl>) -> Option<Decision> {
        self.import_vars(model);

        let mut popper = self.heap.extractor();

        // extract the highest priority variable that is not set yet.
        let next_unset = loop {
            // we are only allowed to remove from the queue variables that are bound/absent.
            // so peek at the next one an only remove it if it was
            match popper.peek() {
                Some(v) => {
                    if model.state.is_bound(v) || model.state.present(v) != Some(true) {
                        // already bound or not present yet, drop the peeked variable before proceeding to next
                        popper.pop().unwrap();
                    } else {
                        // not set, select for decision
                        break Some(v);
                    }
                }
                None => {
                    // no variables left in queue
                    break None;
                }
            }
        };
        if let Some(v) = next_unset {
            if stats.num_conflicts - self.conflicts_at_last_restart >= self.params.allowed_conflicts {
                // we have exceeded the number of allowed conflict, time for a restart
                self.conflicts_at_last_restart = stats.num_conflicts;
                // increase the number of allowed conflicts
                self.params.allowed_conflicts =
                    (self.params.allowed_conflicts as f32 * self.params.increase_ratio_for_allowed_conflicts) as u64;

                Some(Decision::Restart)
            } else {
                // determine value for literal:
                // - first from per-variable preferred assignments
                // - otherwise from the preferred value for boolean variables
                let IntDomain { lb, ub } = model.var_domain(v);
                debug_assert!(lb < ub);

                let value = self
                    .default_assignment
                    .values
                    .get(v)
                    .copied()
                    .unwrap_or(if self.params.prefer_min_value { lb } else { ub });

                let literal = if value < lb || value > ub {
                    if self.params.prefer_min_value {
                        Lit::leq(v, lb)
                    } else {
                        Lit::geq(v, ub)
                    }
                } else if ub > value && self.params.prefer_min_value {
                    Lit::leq(v, value)
                } else if lb < value {
                    Lit::geq(v, value)
                } else {
                    debug_assert!(ub > value);
                    Lit::leq(v, value)
                };

                Some(Decision::SetLiteral(literal))
            }
        } else {
            // all variables are set, no decision left
            None
        }
    }

    pub fn set_default_value(&mut self, var: VarRef, val: IntCst) {
        self.default_assignment.values.insert(var, val);
    }

    /// Increase the activity of the variable and perform an reordering in the queue.
    /// If the variable is optional, the activity of the presence variable is increased as well.
    /// The activity is then used to select the next variable.
    pub fn bump_activity(&mut self, bvar: VarRef, model: &Model<Lbl>) {
        self.heap.var_bump_activity(bvar);
        match model.state.presence(bvar).variable() {
            VarRef::ZERO => {}
            prez_var => self.heap.var_bump_activity(prez_var),
        }
    }
}

impl<Lbl: Label> Default for ActivityBrancher<Lbl> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct BoolHeuristicParams {
    pub var_inc: f32,
    pub var_decay: f32,
}
impl Default for BoolHeuristicParams {
    fn default() -> Self {
        BoolHeuristicParams {
            var_inc: 1_f32,
            var_decay: 0.95_f32,
        }
    }
}

/// Heuristic value associated to a variable.
#[derive(Copy, Clone, PartialEq, PartialOrd)]
struct BoolVarHeuristicValue {
    activity: f32,
}

type Heap = IdxHeap<VarRef, BoolVarHeuristicValue>;

/// Changes that need to be undone.
/// The only change that we need to undo is the removal from the queue.
/// When extracting a variable from the queue, it will be checked whether the variable
/// should be returned to the caller. Thus it is correct to have a variable in the queue
/// that will never be send to a caller.
#[derive(Copy, Clone)]
enum HeapEvent {
    Removal(VarRef, u8),
}

#[derive(Clone)]
pub struct VarSelect {
    params: BoolHeuristicParams,
    /// One heap for each decision stage.
    heaps: Vec<Heap>,
    /// Stage in which each variable appears.
    stages: RefMap<VarRef, u8>,
    trail: Trail<HeapEvent>,
}

impl VarSelect {
    pub fn new(params: BoolHeuristicParams) -> Self {
        VarSelect {
            params,
            heaps: Vec::new(),
            stages: Default::default(),
            trail: Trail::default(),
        }
    }

    pub fn is_declared(&self, v: VarRef) -> bool {
        self.stages.contains(v)
    }

    /// Declares a new variable. The variable is NOT added to the queue.
    /// The stage parameter defines at which stage of the search the variable will be selected.
    /// Variables with the lowest stage are considered first.
    pub fn declare_variable(&mut self, v: VarRef, stage: u8, initial_activity: Option<f32>) {
        debug_assert!(!self.is_declared(v));
        let hvalue = BoolVarHeuristicValue {
            activity: initial_activity.unwrap_or(self.params.var_inc),
        };
        let priority = stage as usize;
        while priority >= self.heaps.len() {
            self.heaps.push(IdxHeap::new());
        }
        self.heaps[priority].declare_element(v, hvalue);
        self.stages.insert(v, priority as u8);
    }

    /// Adds a previously declared variable to its queue
    pub fn enqueue_variable(&mut self, v: VarRef) {
        debug_assert!(self.is_declared(v));
        let priority = self.stages[v] as usize;
        self.heaps[priority].enqueue(v);
    }

    fn stage_of(&self, v: VarRef) -> u8 {
        self.stages[v]
    }

    fn heap_of(&mut self, v: VarRef) -> &mut Heap {
        let heap_index = self.stage_of(v) as usize;
        &mut self.heaps[heap_index]
    }

    /// Provides an iterator over variables in the heap.
    /// Variables are provided by increasing priority.
    pub fn extractor(&mut self) -> Popper {
        let mut heaps = self.heaps.iter_mut();
        let current_heap = heaps.next();
        Popper {
            heaps,
            current_heap,
            stage: 0,
            trail: &mut self.trail,
        }
    }

    pub fn var_bump_activity(&mut self, var: VarRef) {
        let var_inc = self.params.var_inc;
        let heap = self.heap_of(var);
        heap.change_priority(var, |p| p.activity += var_inc);
        if heap.priority(var).activity > 1e30_f32 {
            self.var_rescale_activity()
        }
    }

    pub fn decay_activities(&mut self) {
        self.params.var_inc /= self.params.var_decay;
    }

    fn var_rescale_activity(&mut self) {
        // here we scale the activity of all variables, to avoid overflowing
        // this can not change the relative order in the heap, since activities are scaled by the same amount.
        for heap in &mut self.heaps {
            heap.change_all_priorities_in_place(|p| p.activity *= 1e-30_f32);
        }

        self.params.var_inc *= 1e-30_f32;
    }
}
impl Backtrack for VarSelect {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let heaps = &mut self.heaps;
        self.trail.restore_last_with(|HeapEvent::Removal(var, prio)| {
            heaps[prio as usize].enqueue(var);
        })
    }
}

/// Datastructure that acts as an iterator over a sequence of heaps.
pub struct Popper<'a> {
    heaps: std::slice::IterMut<'a, Heap>,
    current_heap: Option<&'a mut Heap>,
    stage: u8,
    /// An history that records any removal, to ensure that we can backtrack
    /// by putting back any removed elements to the queue.
    trail: &'a mut Trail<HeapEvent>,
}

impl<'a> Popper<'a> {
    /// Returns the next element in the queue, without removing it.
    /// Returns `None` if no elements are left in the queue.
    pub fn peek(&mut self) -> Option<VarRef> {
        while let Some(curr) = &self.current_heap {
            if let Some(var) = curr.peek().copied() {
                return Some(var);
            } else {
                self.current_heap = self.heaps.next();
                self.stage += 1;
            }
        }
        None
    }

    /// Remove the next element from the queue and return it.
    /// Returns `None` if no elements are left in the queue.
    pub fn pop(&mut self) -> Option<VarRef> {
        while let Some(curr) = &mut self.current_heap {
            if let Some(var) = curr.pop() {
                self.trail.push(HeapEvent::Removal(var, self.stage as u8));
                return Some(var);
            } else {
                self.current_heap = self.heaps.next();
                self.stage += 1;
            }
        }
        None
    }
}

impl<Lbl> Backtrack for ActivityBrancher<Lbl> {
    fn save_state(&mut self) -> DecLvl {
        self.heap.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.heap.num_saved()
    }

    fn restore_last(&mut self) {
        self.heap.restore_last()
    }
}

impl<Lbl: Label> SearchControl<Lbl> for ActivityBrancher<Lbl> {
    fn next_decision(&mut self, stats: &Stats, model: &Model<Lbl>) -> Option<Decision> {
        self.next_decision(stats, model)
    }

    fn import_vars(&mut self, model: &Model<Lbl>) {
        self.import_vars(model)
    }

    fn set_default_value(&mut self, var: VarRef, val: IntCst) {
        self.set_default_value(var, val)
    }

    fn new_assignment_found(&mut self, objective: IntCst, assignment: std::sync::Arc<SavedAssignment>) {
        // if we are in LNS mode and the given solution is better than the previous one,
        // set the default value of all variables to the one they have in the solution.
        let is_improvement = self
            .default_assignment
            .objective_found
            .map(|prev| objective < prev)
            .unwrap_or(true);
        if USE_LNS.get() && is_improvement {
            self.default_assignment.objective_found = Some(objective);
            for (var, val) in assignment.bound_variables() {
                self.set_default_value(var, val);
            }
        }
    }

    fn bump_activity(&mut self, bvar: VarRef, model: &Model<Lbl>) {
        self.bump_activity(bvar, model)
    }

    fn decay_activities(&mut self) {
        self.heap.decay_activities()
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Lbl> + Send> {
        Box::new(self.clone())
    }
}
