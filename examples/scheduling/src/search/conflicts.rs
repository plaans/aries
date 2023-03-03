use crate::Var;
use aries::backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail};
use aries::collections::heap::IdxHeap;
use aries::collections::ref_store::{RefMap, RefVec};
use aries::core::literals::{LitSet, Watches};
use aries::core::state::{Conflict, Event, Explainer, IntDomain};
use aries::core::{IntCst, Lit, VarRef};
use aries::model::extensions::{AssignmentExt, SavedAssignment};
use aries::model::Model;
use aries::solver::solver::search::{Decision, SearchControl};
use aries::solver::solver::stats::Stats;
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::Debug;

#[derive(Default, Clone)]
struct ConflictTracking {
    num_conflicts: u64,
    assignment_time: RefMap<VarRef, u64>,
    conflict_since_assignment: RefVec<VarRef, u64>,
    assignments: Trail<VarRef>,
}

/// A branching scheme that first select variables that were recently involved in conflicts.
#[derive(Clone)]
pub struct ConflictBasedBrancher {
    heap: VarSelect,
    default_assignment: PreferredValues,
    num_processed_var: usize,
    /// Associates presence literals to the optional variables
    /// Essentially a Map<Lit, Set<VarRef>>
    presences: Watches<VarRef>,
    cursor: ObsTrailCursor<Event>,
    pub params: Params,
    conflicts: ConflictTracking,
}

/// Optionally associates to each variable a value that is its preferred one.
#[derive(Clone, Default)]
struct PreferredValues {
    /// If these default values came from a valid assignment, this is the value of the associated objective
    objective_found: Option<IntCst>,
    /// Default value for variables (some variables might not have one)
    values: RefMap<VarRef, IntCst>,
}

#[derive(Copy, Clone)]
pub struct Params {
    /// Which scheme to use to define the priority of a variable, based on the conflict it participates in.
    pub heuristic: Heuristic,
    /// How do we determine that a variable participates in a conflict.
    pub active: ActiveLiterals,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Heuristic {
    /// Priority is the activity of the variable: its activity is bumped each time it participates in a conflict.
    Vsids,
    /// Priority is the learning rate of the variable: ration of conflict in which it participates when set
    /// Ref: Learning Rate Based Branching Heuristic for SAT Solvers
    LearningRate,
}
/// Controls which literals are considered active in a conflict.
/// Ref: Learning Rate Based Branching Heuristic for SAT Solvers
#[derive(PartialOrd, PartialEq, Eq, Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum ActiveLiterals {
    /// Only literals of the clause are considered active
    Clause,
    /// Literals of the clause + literals resolved when producing the clause
    Resolved,
    /// Literals of the clause + literals resolved when producing the clause + literals that
    /// appear in the explanation of the literals in teh clause.
    Reasoned,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            active: ActiveLiterals::Reasoned,
            heuristic: Heuristic::LearningRate,
        }
    }
}

impl ConflictBasedBrancher {
    pub fn new() -> Self {
        Self::with(Params::default())
    }

    pub fn with(params: Params) -> Self {
        ConflictBasedBrancher {
            params,
            heap: VarSelect::new(Default::default()),
            default_assignment: PreferredValues::default(),
            num_processed_var: 0,
            presences: Default::default(),
            cursor: ObsTrailCursor::new(),
            conflicts: Default::default(),
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

        self.process_events(model);
    }

    fn is_decision_variable(&self, v: VarRef) -> bool {
        self.heap.is_declared(v)
    }

    fn process_events(&mut self, model: &Model<Var>) {
        // process all new events and enqueue the variables that became present
        while let Some(x) = self.cursor.pop(model.state.trail()) {
            for var in self.presences.watches_on(x.new_literal()) {
                self.heap.enqueue_variable(var);
            }
            let v = x.affected_bound.variable();
            if self.is_decision_variable(x.affected_bound.variable()) {
                // TODO: this assume the variable is binary (and thus set on the first event)
                self.conflicts.conflict_since_assignment.fill_with(v, || 0);
                assert!(!self.conflicts.assignment_time.contains(v));
                self.conflicts.assignment_time.insert(v, self.conflicts.num_conflicts);
                self.conflicts.conflict_since_assignment[v] = 0;
                self.conflicts.assignments.trail.push(v);
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

        // extract the highest priority variable that is not set yet.
        let next_unset = loop {
            // we are only allowed to remove from the queue variables that are bound/absent.
            // so peek at the next one an only remove it if it was
            match self.heap.peek() {
                Some(v) => {
                    if model.state.is_bound(v) || model.state.present(v) != Some(true) {
                        // already bound or not present yet, drop the peeked variable before proceeding to next
                        self.heap.pop().unwrap();
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
            // println!("dec: {literal:?}   {}", self.heap.activity(literal));
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
    pub fn bump_activity(&mut self, lit: Lit, model: &Model<Var>) {
        self.heap.lit_bump_activity(lit);
        let prez = model.state.presence(lit.variable());
        if prez.variable() != VarRef::ZERO {
            self.heap.lit_bump_activity(prez)
        }
    }
}

impl Default for ConflictBasedBrancher {
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

type Heap = IdxHeap<VarRef, f32>;

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
struct VarSelect {
    params: BoolHeuristicParams,
    /// Heap where pending variables are present and allows extracting the highest-priority variable.
    heap: Heap,
    /// Decision variables among which we must select.
    vars: HashSet<VarRef>,
    /// Trail that record events and allows repopulating the heap when backtracking.
    trail: Trail<HeapEvent>,
}

impl VarSelect {
    pub fn new(params: BoolHeuristicParams) -> Self {
        VarSelect {
            params,
            heap: Default::default(),
            vars: Default::default(),
            trail: Trail::default(),
        }
    }

    pub fn is_declared(&self, v: VarRef) -> bool {
        self.vars.contains(&v)
    }

    /// Declares a new variable. The variable is NOT added to the queue.
    /// The stage parameter defines at which stage of the search the variable will be selected.
    /// Variables with the lowest stage are considered first.
    pub fn declare_variable(&mut self, v: VarRef, initial_priority: Option<f32>) {
        debug_assert!(!self.is_declared(v));
        let priority = initial_priority.unwrap_or(0.0);

        self.heap.declare_element(v, priority);
        self.vars.insert(v);
    }

    /// Adds a previously declared variable to its queue
    pub fn enqueue_variable(&mut self, v: VarRef) {
        debug_assert!(self.is_declared(v));
        self.heap.enqueue(v);
    }

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

    pub fn lit_bump_activity(&mut self, lit: Lit) {
        self.lit_increase_activity(lit, 1.0)
    }

    fn lit_increase_activity(&mut self, lit: Lit, factor: f32) {
        let var = lit.variable();
        if self.vars.contains(&var) {
            let var_inc = self.params.var_inc * factor;

            self.heap.change_priority(var, |p| *p += var_inc);
            let p = self.heap.priority(var);
            if p > 1e30_f32 {
                self.var_rescale_activity()
            }
        }
    }

    pub fn lit_update_activity(&mut self, lit: Lit, new_value: f32, factor: f32, num_decays_to_undo: u32) {
        debug_assert!(!new_value.is_nan());
        debug_assert!(!factor.is_nan());
        let var = lit.variable();
        if self.vars.contains(&var) {
            // assert!(self.params.var_inc == 1.0_f32);

            let factor = factor as f64;
            let new_value = new_value as f64;

            let new_priority = loop {
                let var_inc = self.params.var_inc as f64;
                let previous = self.heap.priority(var) as f64;
                // the value was decayed N times, we undo this by multiplying it by (decay_factor^(-N))
                // this can result in very large numbers, hence the usage of f64 to avoid over shouting
                // to avoid rare case, (1/0.95)^14000 = infinity, we saturate very high
                let correction = (self.params.var_decay as f64)
                    .powi(-(num_decays_to_undo as i32))
                    .min(1e300_f64);
                let corrected = previous * correction;
                // we might loose a lot of precision in the above multiplication, make sure we stay within the normal bounds
                let corrected = corrected.clamp(0.0, var_inc);
                let new = corrected * (1.0 - factor) + new_value * factor * var_inc;
                if new > 1e30_f64 {
                    // the result would not fit in an f32, rescale all variables and repeat
                    // I suspect that in extreme cases, several rescale might be necessary, hence the loop
                    self.var_rescale_activity();
                } else {
                    // we would fit in an f32, proceed with the update
                    break new;
                }
            } as f32;

            debug_assert!((-0.1..=1.1).contains(&(new_priority / self.params.var_inc)));
            self.heap.change_priority(var, |p| *p = new_priority);
        }
    }

    /// Reduce the activity of all variables by a constant factor
    pub fn decay_activities(&mut self) {
        // instead of reducing the the priority of all variables, we increase the reference.
        // This way a previous activity bump is worth less than before. On the other hand,
        // future increases will be based on the reference. `var_inc`
        self.params.var_inc /= self.params.var_decay;
    }

    fn var_rescale_activity(&mut self) {
        // here we scale the activity of all variables, to avoid overflowing
        // this can not change the relative order in the heap, since activities are scaled by the same amount.
        self.heap.change_all_priorities_in_place(|p| {
            *p *= 1e-30_f32;
        });
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

impl Backtrack for ConflictBasedBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.conflicts.assignments.save_state();
        self.heap.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.heap.num_saved()
    }

    fn restore_last(&mut self) {
        self.conflicts.assignments.restore_last_with(|v| {
            let tot = self.conflicts.num_conflicts - self.conflicts.assignment_time[v];
            let involved = self.conflicts.conflict_since_assignment[v];
            // println!("{v:?}: {involved} / {tot}     {}", self.conflicts.num_conflicts);
            self.conflicts.assignment_time.remove(v);
            let lr = (involved as f32) / (tot as f32);
            if self.params.heuristic == Heuristic::LearningRate && !lr.is_nan() {
                self.heap.lit_update_activity(v.geq(1), lr, 0.05_f32, tot as u32);
            }
        });
        self.heap.restore_last()
    }
}

impl SearchControl<Var> for ConflictBasedBrancher {
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

    fn conflict(&mut self, clause: &Conflict, model: &Model<Var>, _explainer: &mut dyn Explainer) {
        self.conflicts.num_conflicts += 1;
        // bump activity of all variables of the clause
        self.heap.decay_activities();

        let mut culprits = LitSet::new();
        for b in clause.literals() {
            culprits.insert(!*b);
        }
        if self.params.active >= ActiveLiterals::Resolved {
            for l in clause.resolved.literals() {
                culprits.insert(l);
            }
        }
        if self.params.active >= ActiveLiterals::Reasoned {
            for disjunct in clause.literals() {
                let l = !*disjunct;
                if model.entails(l) {
                    if let Some(reasons) = model.state.implying_literals(l, _explainer) {
                        for r in reasons {
                            debug_assert!(model.entails(r));
                            culprits.insert(r);
                        }
                    }
                }
            }
        }

        // we have identified all culprits, update the heuristic information (depending on the heuristic used)
        for culprit in culprits.literals() {
            match self.params.heuristic {
                Heuristic::Vsids => {
                    // bump activity of all culprits
                    self.bump_activity(culprit, model);
                }
                Heuristic::LearningRate => {
                    // learning rate branching, record that the variable participated in thus conflict
                    // the variable's priority will be updated upon backtracking
                    let v = culprit.variable();
                    if self.is_decision_variable(v) {
                        // println!("  culprit: {v:?}  {:?}  ", model.value_of_literal(culprit),);
                        debug_assert!(self.conflicts.assignment_time[v] <= self.conflicts.num_conflicts);
                        debug_assert!(
                            self.conflicts.num_conflicts - self.conflicts.assignment_time[v]
                                > self.conflicts.conflict_since_assignment[v]
                        );
                        self.conflicts.conflict_since_assignment[v] += 1;
                    }
                }
            }
        }
    }
    fn pre_conflict_analysis(&mut self, _model: &Model<Var>) {
        self.process_events(_model);
    }

    fn pre_save_state(&mut self, _model: &Model<Var>) {
        // TODO: merge with save state
        // println!("PRE SAVE");
        self.process_events(_model);
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}
