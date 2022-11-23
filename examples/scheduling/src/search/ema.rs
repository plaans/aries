use crate::Var;
use aries_backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail};
use aries_collections::heap::IdxHeap;
use aries_collections::ref_store::RefMap;
use aries_core::literals::{Disjunction, Watches};
use aries_core::state::{Event, Explainer, IntDomain};
use aries_core::{IntCst, Lit, VarRef};
use aries_model::extensions::{AssignmentExt, SavedAssignment};
use aries_model::Model;
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
use itertools::Itertools;
use std::collections::HashSet;

/// A branching scheme that first select variables that were recently involved in conflicts.
#[derive(Clone)]
pub struct EMABrancher {
    heap: VarSelect,
    default_assignment: DefaultValues,
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

impl EMABrancher {
    pub fn new() -> Self {
        EMABrancher {
            heap: VarSelect::new(Default::default()),
            default_assignment: DefaultValues::default(),
            num_processed_var: 0,
            presences: Default::default(),
            cursor: ObsTrailCursor::new(),
        }
    }

    pub fn import_vars(&mut self, model: &Model<Var>) {
        let mut count = 0;
        // go through the model's variables and declare any newly declared variable
        // TODO: use `advance_by` when it is stabilized. The current usage of `dropping` is very expensive
        //       when compiled without opt-level=3. advance_by should be easier to optimize but dropping will
        //       have to wait to adopt it. [Tracking issue](https://github.com/rust-lang/rust/issues/77404)
        for var in model.state.variables().dropping(self.num_processed_var) {
            debug_assert!(!self.heap.is_declared(var));
            if let Some(Var::Prec(_, _, _, _)) = model.shape.labels.get(var) {
                let prez = model.presence_literal(var);
                self.heap.declare_variable(var, None);
                // remember that, when `prez` becomes true we must enqueue the variable
                self.presences.add_watch(var, prez);

                // `prez` is already true, enqueue the variable immediately
                if model.entails(prez) {
                    self.heap.enqueue_variable(var);
                }
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
    pub fn next_decision(&mut self, _stats: &Stats, model: &Model<Var>) -> Option<Decision> {
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
            // determine value for literal:
            // - first from per-variable preferred assignments
            // - otherwise from the preferred value for boolean variables
            let IntDomain { lb, ub } = model.var_domain(v);
            debug_assert!(lb < ub);

            let value = self.default_assignment.values.get(v).copied().unwrap_or(lb);

            let literal = if value <= lb {
                Lit::leq(v, lb)
            } else if value >= ub {
                Lit::geq(v, ub)
            } else {
                Lit::leq(v, value)
            };

            Some(Decision::SetLiteral(literal))
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
    pub fn bump_activity(&mut self, bvar: VarRef, model: &Model<Var>) {
        self.heap.var_bump_activity(bvar);
        match model.state.presence(bvar).variable() {
            VarRef::ZERO => {}
            prez_var => self.heap.var_bump_activity(prez_var),
        }
    }
}

impl Default for EMABrancher {
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
    Removal(VarRef),
}

#[derive(Clone)]
pub struct VarSelect {
    params: BoolHeuristicParams,
    /// One heap for each decision stage.
    heap: Heap,
    /// Stage in which each variable appears.
    stages: HashSet<VarRef>,
    trail: Trail<HeapEvent>,
}

impl VarSelect {
    pub fn new(params: BoolHeuristicParams) -> Self {
        VarSelect {
            params,
            heap: Default::default(),
            stages: Default::default(),
            trail: Trail::default(),
        }
    }

    pub fn is_declared(&self, v: VarRef) -> bool {
        self.stages.contains(&v)
    }

    /// Declares a new variable. The variable is NOT added to the queue.
    /// The stage parameter defines at which stage of the search the variable will be selected.
    /// Variables with the lowest stage are considered first.
    pub fn declare_variable(&mut self, v: VarRef, initial_activity: Option<f32>) {
        debug_assert!(!self.is_declared(v));
        let hvalue = BoolVarHeuristicValue {
            activity: initial_activity.unwrap_or(self.params.var_inc),
        };

        self.heap.declare_element(v, hvalue);
        self.stages.insert(v);
    }

    /// Adds a previously declared variable to its queue
    pub fn enqueue_variable(&mut self, v: VarRef) {
        debug_assert!(self.is_declared(v));
        self.heap.enqueue(v);
    }

    /// Provides an iterator over variables in the heap.
    /// Variables are provided by increasing priority.
    pub fn extractor(&mut self) -> Popper {
        Popper {
            heap: &mut self.heap,
            trail: &mut self.trail,
        }
    }

    pub fn var_bump_activity(&mut self, var: VarRef) {
        if self.stages.contains(&var) {
            let var_inc = self.params.var_inc;
            self.heap.change_priority(var, |p| p.activity += var_inc);
            if self.heap.priority(var).activity > 1e30_f32 {
                self.var_rescale_activity()
            }
        }
    }

    // pub fn activity(&self, var: VarRef) -> f32 {
    //     self.heap.priority(var).activity / self.params.var_inc
    // }

    pub fn decay_activities(&mut self) {
        self.params.var_inc /= self.params.var_decay;
    }

    fn var_rescale_activity(&mut self) {
        // here we scale the activity of all variables, to avoid overflowing
        // this can not change the relative order in the heap, since activities are scaled by the same amount.
        self.heap.change_all_priorities_in_place(|p| p.activity *= 1e-30_f32);
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
        self.trail.restore_last_with(|HeapEvent::Removal(var)| {
            self.heap.enqueue(var);
        })
    }
}

/// Datastructure that acts as an iterator over a sequence of heaps.
pub struct Popper<'a> {
    heap: &'a mut Heap,
    /// An history that records any removal, to ensure that we can backtrack
    /// by putting back any removed elements to the queue.
    trail: &'a mut Trail<HeapEvent>,
}

impl<'a> Popper<'a> {
    /// Returns the next element in the queue, without removing it.
    /// Returns `None` if no elements are left in the queue.
    pub fn peek(&mut self) -> Option<VarRef> {
        self.heap.peek().copied()
    }

    /// Remove the next element from the queue and return it.
    /// Returns `None` if no elements are left in the queue.
    pub fn pop(&mut self) -> Option<VarRef> {
        if let Some(var) = self.heap.pop() {
            self.trail.push(HeapEvent::Removal(var));
            Some(var)
        } else {
            None
        }
    }
}

impl Backtrack for EMABrancher {
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

impl SearchControl<Var> for EMABrancher {
    fn next_decision(&mut self, stats: &Stats, model: &Model<Var>) -> Option<Decision> {
        self.next_decision(stats, model)
    }

    fn import_vars(&mut self, model: &Model<Var>) {
        self.import_vars(model)
    }

    fn new_assignment_found(&mut self, objective: IntCst, assignment: std::sync::Arc<SavedAssignment>) {
        // if we are in LNS mode and the given solution is better than the previous one,
        // set the default value of all variables to the one they have in the solution.
        let is_improvement = self
            .default_assignment
            .objective_found
            .map(|prev| objective < prev)
            .unwrap_or(true);
        if is_improvement {
            self.default_assignment.objective_found = Some(objective);
            for (var, val) in assignment.bound_variables() {
                self.set_default_value(var, val);
            }
        }
    }

    fn conflict(&mut self, clause: &Disjunction, model: &Model<Var>, _explainer: &mut dyn Explainer) {
        // bump activity of all variables of the clause
        self.heap.decay_activities();
        for b in clause.literals() {
            self.bump_activity(b.variable(), model);
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}
