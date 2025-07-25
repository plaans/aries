use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};
use std::sync::Arc;

use aries::model::lang::*;

pub type Sym = String;
pub type Time = FAtom;

/// A fluent is a state function defined as a symbol and a set of parameter and return types.
///
/// For instance `at: Robot -> Location -> Bool` is the state function with symbol `at`
/// that accepts two parameters of type `Robot` and `Location`.
///
/// Given two symbols `bob: Robot` and `kitchen: Location`, the application of the
/// *state function* `at` to these parameters:
/// `(at bob kitchen)` is a *state variable* of boolean type.
// TODO: make internals private
#[derive(Clone, Debug, Eq, PartialOrd, Ord)]
pub struct Fluent {
    /// Human readable name of the fluent
    pub sym: Sym,
    /// Signature of the function. A vec [a, b, c] corresponds
    /// to the type `a -> b -> c` in curried notation.
    /// Hence `a` and `b` are the arguments and the last element `c` is the return type
    pub signature: Vec<Type>,
}
impl PartialEq for Fluent {
    fn eq(&self, other: &Self) -> bool {
        // if they have the same symbol they should be exactly the same by construct
        debug_assert!(
            self.sym != other.sym || self.signature == other.signature,
            "{:?} {:?}",
            self,
            other
        );
        self.sym == other.sym
    }
}
impl Hash for Fluent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sym.hash(state);
    }
}
/// A state variable e.g. `(location-of robot1)` where:
///  - the fluent is the name of the state variable (e.g. `location-of`) and defines its type.
///  - the remaining elements are its parameters (e.g. `robot1`).
#[derive(Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct StateVar {
    pub fluent: Arc<Fluent>,
    pub args: Vec<SAtom>,
}
impl StateVar {
    pub fn new(fluent: Arc<Fluent>, args: Vec<SAtom>) -> Self {
        StateVar { fluent, args }
    }
}
impl Debug for StateVar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.fluent)?;
        f.debug_list().entries(self.args.iter()).finish()
    }
}

/// Represents an effect on a state variable.
/// The effect has a first transition phase `]transition_start, transition_end[` during which the
/// value of the state variable is unknown.
/// Exactly at time `transition_end`, the state variable `state_var` is update with `value`
/// (assignment or increase based on `operation`).
/// For assignment effects, this value will persist until another assignment effect starts its own transition.
#[derive(Clone, Eq, PartialEq)]
pub struct Effect {
    /// Time at which the transition to the new value will start
    pub transition_start: Time,
    /// Time at which the transition will end
    pub transition_end: Time,
    /// If specified, the assign effect is required to persist at least until all of these timepoints.
    pub min_mutex_end: Vec<Time>,
    /// State variable affected by the effect
    pub state_var: StateVar,
    /// Operation carried out by the effect (value assignment, increase)
    pub operation: EffectOp,
}
#[derive(Clone, Eq, PartialEq)]
pub enum EffectOp {
    Assign(Atom),
}
impl EffectOp {
    pub const TRUE_ASSIGNMENT: EffectOp = EffectOp::Assign(Atom::TRUE);
    pub const FALSE_ASSIGNMENT: EffectOp = EffectOp::Assign(Atom::FALSE);
}
impl Debug for EffectOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectOp::Assign(val) => {
                write!(f, ":= {val:?}")
            }
        }
    }
}

impl Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}, {:?}] {:?} {:?}",
            self.transition_start, self.transition_end, self.state_var, self.operation
        )
    }
}

impl Effect {
    pub fn effective_start(&self) -> Time {
        self.transition_end
    }
    pub fn transition_start(&self) -> Time {
        self.transition_start
    }
    pub fn variable(&self) -> &StateVar {
        &self.state_var
    }
}
/// A condition stating that the state variable `state_var` should have the value `value`
/// over the `[start,end]` temporal interval.
///
/// in ANML: `[start,end] state_var == value`
#[derive(Clone)]
pub struct Condition {
    pub start: Time,
    pub end: Time,
    pub state_var: StateVar,
    pub value: Atom,
}

/// Task
#[derive(Clone)]
pub struct Task {
    /// An optional identifier for the task that allows referring to it unambiguously.
    pub name: Sym,
    /// Time reference at which the task must start
    pub start: Time,
    /// Time reference at which the task must end
    pub end: Time,
    /// Arguments of the task
    pub args: Vec<Atom>,
    /// Presence of the task, true iff it appears in the solution
    pub presence: Lit,
}
impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?},{:?}] {:?}{:?}", self.start, self.end, self.name, self.args)?;
        Ok(())
    }
}

pub struct Model {
    origin: Time,
    horizon: Time,
    makespan: Time,
    tasks: Vec<Task>,
    conditions: Vec<Condition>,
    effects: Vec<Effect>,
}
