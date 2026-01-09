pub mod assignment;
pub mod boxes;
pub mod constraints;
mod effects;
pub mod explain;
pub mod symbols;
pub mod tasks;

use aries::model::extensions::DomainsExt;
use constraints::*;
use core::fmt::Debug;
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
pub use crate::effects::*;
use crate::explain::ExplainableSolver;
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

#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub enum Tag {
    TaskStart(TaskId),
    TaskEnd(TaskId),
}

type Constraint = Box<dyn BoolExpr<Sched>>;
pub type ConstraintID = usize;

pub struct Sched {
    pub model: Model,
    pub objects: ObjectEncoding,
    pub time_scale: IntCst,
    pub origin: Time,
    pub horizon: Time,
    pub makespan: Time,
    pub tasks: Tasks,
    pub effects: Effects,
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

    pub fn add_effect(&mut self, eff: Effect) -> EffectId {
        self.effects.add_effect(eff, |var| self.model.int_bounds(var))
    }

    pub fn new_timepoint(&mut self) -> Time {
        self.model.new_fvar(0, INT_CST_MAX, self.time_scale, "_").into()
    }
    pub fn new_opt_timepoint(&mut self, scope: Lit) -> Time {
        self.model
            .new_optional_fvar(0, INT_CST_MAX, self.time_scale, scope, "_")
            .into()
    }
    pub fn add_constraint<C: BoolExpr<Sched> + 'static>(&mut self, c: C) -> ConstraintID {
        self.add_boxed_constraint(Box::new(c))
    }
    pub fn add_boxed_constraint(&mut self, c: Box<dyn BoolExpr<Sched> + 'static>) -> ConstraintID {
        self.constraints.push(c);
        self.constraints.len() - 1
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

    pub fn explainable_solver<T: Ord + Clone>(
        &self,
        project: impl Fn(ConstraintID) -> Option<T>,
    ) -> ExplainableSolver<T> {
        ExplainableSolver::new(self, project)
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
