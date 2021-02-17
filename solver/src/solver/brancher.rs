use crate::solver::stats::Stats;
use aries_backtrack::{Backtrack, DecLvl, Trail};
use aries_collections::heap::IdxHeap;
use aries_model::assignments::Assignment;
use env_param::EnvParam;

use aries_collections::ref_store::RefMap;
use aries_model::int_model::IntDomain;

use aries_model::bounds::Bound;
use aries_model::lang::{IntCst, VarRef};
use aries_model::Model;
use itertools::Itertools;

pub static PREFER_MIN_VALUE: EnvParam<bool> = EnvParam::new("ARIES_SMT_PREFER_MIN_VALUE", "true");
pub static INITIALLY_ALLOWED_CONFLICTS: EnvParam<u64> = EnvParam::new("ARIES_SMT_INITIALLY_ALLOWED_CONFLICT", "100");
pub static INCREASE_RATIO_FOR_ALLOWED_CONFLICTS: EnvParam<f32> =
    EnvParam::new("ARIES_SMT_INCREASE_RATIO_FOR_ALLOWED_CONFLICTS", "1.5");

pub struct BranchingParams {
    pub prefer_min_value: bool,
    pub allowed_conflicts: u64,
    pub increase_ratio_for_allowed_conflicts: f32,
}

impl Default for BranchingParams {
    fn default() -> Self {
        BranchingParams {
            prefer_min_value: *PREFER_MIN_VALUE.get(),
            allowed_conflicts: *INITIALLY_ALLOWED_CONFLICTS.get(),
            increase_ratio_for_allowed_conflicts: *INCREASE_RATIO_FOR_ALLOWED_CONFLICTS.get(),
        }
    }
}

pub struct Brancher {
    pub params: BranchingParams,
    bool_sel: BoolVarSelect,
    default_assignment: DefaultValues,
    trail: Trail<UndoChange>,
    conflicts_at_last_restart: u64,
    num_processed_var: usize,
}

#[derive(Default)]
struct DefaultValues {
    bools: RefMap<VarRef, IntCst>,
}

/// Changes that need to be undone.
/// The only change that we need to undo is the removal from the queue.
/// When extracting a variable from the queue, it will be checked whether the variable
/// should be returned to the caller. Thus it is correct to have a variable in the queue
/// that will never be send to a caller.
enum UndoChange {
    Removal(VarRef),
}

pub enum Decision {
    SetLiteral(Bound),
    Restart,
}

impl Brancher {
    pub fn new() -> Self {
        Brancher {
            params: Default::default(),
            bool_sel: BoolVarSelect::new(Default::default()),
            default_assignment: DefaultValues::default(),
            trail: Default::default(),
            conflicts_at_last_restart: 0,
            num_processed_var: 0,
        }
    }

    fn import_vars(&mut self, model: &Model) {
        let mut count = 0;
        for var in model.discrete.variables().dropping(self.num_processed_var) {
            debug_assert!(!self.bool_sel.is_declared(var));
            let priority = if model.var_domain(var).size() <= 1 { 9 } else { 0 };
            self.bool_sel.declare_variable(var, priority);
            self.bool_sel.enqueue_variable(var);
            count += 1;
        }
        self.num_processed_var += count;
    }

    /// Select the next decision to make.
    /// Returns `None` if no decision is left to be made.
    pub fn next_decision(&mut self, stats: &Stats, model: &Model) -> Option<Decision> {
        self.import_vars(model);

        // extract the highest priority variable that is not set yet.
        let next_unset = loop {
            match self.bool_sel.peek_next_var() {
                Some(v) => {
                    if model.var_domain(v).is_bound() {
                        // already bound, drop the peeked variable before proceeding to next
                        let v = self.bool_sel.pop_next_var().unwrap();
                        self.trail.push(UndoChange::Removal(v));
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
                    .bools
                    .get(v)
                    .copied()
                    .unwrap_or(if self.params.prefer_min_value { lb } else { ub });

                let literal = if value < lb || value > ub {
                    if self.params.prefer_min_value {
                        Bound::leq(v, lb)
                    } else {
                        Bound::geq(v, ub)
                    }
                } else if ub > value && self.params.prefer_min_value {
                    Bound::leq(v, value)
                } else if lb < value {
                    Bound::geq(v, value)
                } else {
                    debug_assert!(ub > value);
                    Bound::leq(v, value)
                };

                Some(Decision::SetLiteral(literal))
            }
        } else {
            // all variables are set, no decision left
            None
        }
    }

    pub fn set_default_value(&mut self, var: VarRef, val: IntCst) {
        self.default_assignment.bools.insert(var, val);
    }

    pub fn set_default_values_from(&mut self, assignment: &Model) {
        self.import_vars(assignment);
        for (var, val) in assignment.discrete.bound_variables() {
            self.set_default_value(var, val);
        }
    }

    /// Increase the activity of the variable and perform an reordering in the queue.
    /// The activity is then used to select the next variable.
    pub fn bump_activity(&mut self, bvar: VarRef) {
        self.bool_sel.var_bump_activity(bvar);
    }
}

impl Default for Brancher {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BoolHeuristicParams {
    pub var_inc: f64,
    pub var_decay: f64,
}
impl Default for BoolHeuristicParams {
    fn default() -> Self {
        BoolHeuristicParams {
            var_inc: 1_f64,
            var_decay: 0.95_f64,
        }
    }
}

/// Heuristic value associated to a variable.
#[derive(Copy, Clone, PartialEq, PartialOrd)]
struct BoolVarHeuristicValue {
    priority: u8,
    activity: f64,
}

pub struct BoolVarSelect {
    params: BoolHeuristicParams,
    heap: IdxHeap<VarRef, BoolVarHeuristicValue>,
}

impl BoolVarSelect {
    pub fn new(params: BoolHeuristicParams) -> Self {
        BoolVarSelect {
            params,
            heap: IdxHeap::new(),
        }
    }

    pub fn is_declared(&self, v: VarRef) -> bool {
        self.heap.is_declared(v)
    }

    /// Declares a new variable. The variable is NOT added to the queue.
    pub fn declare_variable(&mut self, v: VarRef, priority: u8) {
        let hvalue = BoolVarHeuristicValue {
            priority,
            activity: self.params.var_inc,
        };
        self.heap.declare_element(v, hvalue);
    }

    /// Add the value to the queue, the variable must have been previously declared.
    pub fn enqueue_variable(&mut self, var: VarRef) {
        self.heap.enqueue(var)
    }

    pub fn pop_next_var(&mut self) -> Option<VarRef> {
        self.heap.pop()
    }

    pub fn peek_next_var(&mut self) -> Option<VarRef> {
        self.heap.peek().copied()
    }

    pub fn var_bump_activity(&mut self, var: VarRef) {
        let var_inc = self.params.var_inc;
        self.heap.change_priority(var, |p| p.activity += var_inc);
        if self.heap.priority(var).activity > 1e100_f64 {
            self.var_rescale_activity()
        }
    }

    pub fn decay_activities(&mut self) {
        self.params.var_inc /= self.params.var_decay;
    }

    fn var_rescale_activity(&mut self) {
        // here we scale the activity of all variables, to avoid overflowing
        // this can not change the relative order in the heap, since activities are scaled by the same amount.
        self.heap.change_all_priorities_in_place(|p| p.activity *= 1e-100_f64);
        self.params.var_inc *= 1e-100_f64;
    }
}

impl Backtrack for Brancher {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let bools = &mut self.bool_sel;
        self.trail.restore_last_with(|event| match event {
            UndoChange::Removal(x) => {
                bools.enqueue_variable(x);
            }
        })
    }
}
