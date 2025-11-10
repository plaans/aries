pub mod assignment;
pub mod constraints;
pub mod symbols;
pub mod tasks;

use constraints::*;
use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};
use std::collections::HashMap;

use aries::core::INT_CST_MAX;
pub use aries::core::IntCst;
use aries::model::lang::hreif::BoolExpr;
use aries::model::lang::*;
use aries::solver::Solver;
use idmap::DirectIdMap;
use itertools::Itertools;

pub type Model = aries::model::Model<Sym>;
use crate::assignment::Assignment;
use crate::symbols::ObjectEncoding;
pub use crate::tasks::*;

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

pub type SymAtom = IAtom;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct StateVar {
    pub fluent: Sym,
    pub args: Vec<SymAtom>,
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
    pub mutex_end: Time,
    /// State variable affected by the effect
    pub state_var: StateVar,
    /// Operation carried out by the effect (value assignment, increase)
    pub operation: EffectOp,
    /// Presence literal indicating whether the effect is present
    pub prez: Lit,
}
#[derive(Clone, Eq, PartialEq)]
pub enum EffectOp {
    Assign(bool),
}
impl EffectOp {
    // pub const TRUE_ASSIGNMENT: EffectOp = EffectOp::Assign(Atom::TRUE);
    // pub const FALSE_ASSIGNMENT: EffectOp = EffectOp::Assign(Atom::FALSE);
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

#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub enum Tag {
    TaskStart(TaskId),
    TaskEnd(TaskId),
}

type Constraint = Box<dyn BoolExpr<Sched>>;

pub struct Sched {
    pub model: Model,
    pub objects: ObjectEncoding,
    time_scale: IntCst,
    pub origin: Time,
    pub horizon: Time,
    pub makespan: Time,
    pub tasks: Tasks,
    pub effects: Vec<Effect>,
    tags: HashMap<Atom, Vec<Tag>>,
    constraints: Vec<Constraint>,
}

impl Sched {
    pub fn new(time_scale: IntCst, objects: ObjectEncoding) -> Self {
        let mut model = Model::new();
        let origin = Time::new(0.into(), time_scale);
        let horizon = model.new_fvar(0, INT_CST_MAX, time_scale, "horizon").into();
        let makespan = model.new_fvar(0, INT_CST_MAX, time_scale, "makespan").into();
        Sched {
            model,
            objects,
            time_scale,
            origin,
            horizon,
            makespan,
            tasks: Default::default(),
            effects: Default::default(),
            tags: Default::default(),
            constraints: vec![Box::new(EffectCoherence)], // TODO: add default constraints (consitency, makespan), ...
        }
    }

    pub fn tag(&mut self, atom: impl Into<Atom>, tag: Tag) {
        let atom = atom.into();
        self.tags.entry(atom).or_default().push(tag);
    }

    pub fn add_task(&mut self, task: Task) -> TaskId {
        self.tasks.insert(task)
    }

    pub fn new_timepoint(&mut self) -> Time {
        self.model.new_fvar(0, INT_CST_MAX, self.time_scale, "_").into()
    }
    pub fn new_opt_timepoint(&mut self, scope: Lit) -> Time {
        self.model
            .new_optional_fvar(0, INT_CST_MAX, self.time_scale, scope, "_")
            .into()
    }
    pub fn add_constraint<C: BoolExpr<Sched> + 'static>(&mut self, c: C) {
        self.add_boxed_constraint(Box::new(c));
    }
    pub fn add_boxed_constraint(&mut self, c: Box<dyn BoolExpr<Sched> + 'static>) {
        self.constraints.push(c);
    }
    pub fn encode(&self) -> Model {
        let mut encoding = self.model.clone();
        for c in &self.constraints {
            c.enforce(self, &mut encoding);
        }
        encoding
    }

    pub fn solve(&self) -> Option<Assignment<'static>> {
        let encoding = self.encode();
        let mut solver = Solver::new(encoding);
        solver.solve().unwrap().map(Assignment::shared)
    }

    pub fn print(&self, sol: &Assignment<'_>) {
        let sorted_tasks = self
            .tasks
            .iter()
            .filter(|t| sol.eval(t.presence) == Some(true))
            .sorted_by_cached_key(|t| sol.eval(t.start.num).unwrap());
        for t in sorted_tasks {
            println!("{}: {}", t.name, sol.eval(t.start.num).unwrap())
        }
    }
}
