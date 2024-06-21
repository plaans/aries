use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail};
use crate::collections::heap::IdxHeap;
use crate::collections::ref_store::{RefMap, RefVec};
use crate::core::literals::{LitSet, Watches};
use crate::core::state::{Conflict, Event, Explainer, IntDomain};
use crate::core::{IntCst, Lit, VarRef};
use crate::model::extensions::{AssignmentExt, SavedAssignment};
use crate::model::Model;
use crate::solver::search::{Decision, SearchControl};
use crate::solver::stats::Stats;

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
    /// vars that should be considered for branching but htat have not been processed yet.
    unprocessed_vars: Vec<VarRef>,
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
    /// This is the most effective.
    Reasoned,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            // Reasoned is normally the most efficient but our implementation for explanation is
            // not always supporting it.
            active: ActiveLiterals::Resolved,
            heuristic: Heuristic::LearningRate,
        }
    }
}

impl ConflictBasedBrancher {
    pub fn new(choices: Vec<Lit>) -> Self {
        Self::with(choices, Params::default())
    }

    pub fn with(choices: Vec<Lit>, params: Params) -> Self {
        let vars: HashSet<VarRef> = choices.iter().map(|l| l.variable()).collect();
        ConflictBasedBrancher {
            params,
            heap: VarSelect::new(Default::default()),
            default_assignment: PreferredValues::default(),
            unprocessed_vars: vars.iter().copied().collect(),
            presences: Default::default(),
            cursor: ObsTrailCursor::new(),
            conflicts: Default::default(),
        }
    }

    fn import_vars<Var>(&mut self, model: &Model<Var>) {
        while let Some(var) = self.unprocessed_vars.pop() {
            debug_assert!(!self.heap.is_declared(var));
            let prez = model.presence_literal(var);
            self.heap.declare_variable(var, None);
            // remember that, when `prez` becomes true we must enqueue the variable
            self.presences.add_watch(var, prez);

            // `prez` is already true, enqueue the variable immediately
            if model.entails(prez) {
                self.heap.enqueue_variable(var);
            }
        }

        self.process_events(model);
    }

    fn is_decision_variable(&self, v: VarRef) -> bool {
        self.heap.is_declared(v)
    }

    fn process_events<Var>(&mut self, model: &Model<Var>) {
        // process all new events and enqueue the variables that became present
        while let Some(x) = self.cursor.pop(model.state.trail()) {
            for var in self.presences.watches_on(x.new_literal()) {
                self.heap.enqueue_variable(var);
            }
            let v = x.affected_bound.variable();
            if self.is_decision_variable(x.affected_bound.variable()) {
                // TODO: this assume the variable is binary (and thus set on the first event)
                self.conflicts.conflict_since_assignment.fill_with(v, || 0);
                // if a variable is touched more than once, assume the latest time is the one of interest
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
    pub fn next_decision<Var>(&mut self, _stats: &Stats, model: &Model<Var>) -> Option<Decision> {
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
    pub fn bump_activity<Var>(&mut self, lit: Lit, model: &Model<Var>) {
        self.heap.lit_bump_activity(lit);
        let prez = model.state.presence(lit.variable());
        match prez.variable() {
            VarRef::ZERO => {}
            VarRef::ONE => {}
            _ => self.heap.lit_bump_activity(prez),
        }
    }
}

#[derive(Clone)]
struct BoolHeuristicParams {
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

            // sanity check that the priority update is more or less in [0,1]
            // Not a debug_assert as it can sometime deviates a bit in unpredictable ways
            // and we do not want to bring the whole planner down for these rare cases
            debug_assert!({
                if !(-0.1..=1.1).contains(&(new_priority / self.params.var_inc)) {
                    tracing::warn!(
                        "Out of theoretical bounds priority update: {} / {} = {}",
                        new_priority,
                        self.params.var_inc,
                        new_priority / self.params.var_inc
                    );
                };
                true
            });
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

impl<Var> SearchControl<Var> for ConflictBasedBrancher {
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

    fn pre_save_state(&mut self, _model: &Model<Var>) {
        self.process_events(_model);
    }
    fn pre_conflict_analysis(&mut self, _model: &Model<Var>) {
        self.process_events(_model);
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
                        for &r in &reasons {
                            // the reason is expected to be entailed, but it may not if it serves as a (presence) guard
                            // of another literal of an explanation.
                            // Note that after a few explanations, the guard may be indirect (i.e. a literal that implies the presence)
                            // so we cannot explicitly check that it is the presence of another literal in the clause.
                            // We only check that the non-entailed literal may be a presence literal (i.e. always present).
                            debug_assert!(model.entails(r) || model.presence_literal(r.variable()) == Lit::TRUE);
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
                        // TODO: reactivate those checks and investigating as for why they may fail
                        // debug_assert!(dbg!(self.conflicts.assignment_time[v]) <= dbg!(self.conflicts.num_conflicts));
                        // debug_assert!(
                        //     self.conflicts.num_conflicts - self.conflicts.assignment_time[v]
                        //         > dbg!(self.conflicts.conflict_since_assignment[v])
                        // );
                        self.conflicts.conflict_since_assignment[v] += 1;
                    }
                }
            }
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}
