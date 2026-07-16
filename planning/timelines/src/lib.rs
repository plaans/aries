pub mod boxes;
pub mod constraints;
mod effects;
pub mod encoder;
pub mod explain;
pub mod symbols;
pub mod tasks;

use aries_solver::core::state::Evaluable;
use aries_solver::core::views::Dom;
use constraints::*;
use core::fmt::Debug;
use core::hash::Hash;
use std::sync::Arc;

use aries_solver::core::INT_CST_MAX;
pub use aries_solver::core::IntCst;
use aries_solver::lang::*;
use aries_solver::prelude::*;
use aries_solver::solver::Solver;
use idmap::DirectIdMap;
use itertools::Itertools;

pub type Model = aries_solver::model::Model<Sym>;
pub use crate::effects::*;
use crate::encoder::{CausalLinks, SchedEncoder};
use crate::explain::ExplainableSolver;
use crate::symbols::ObjectEncoding;
pub use crate::tasks::*;

pub type Sym = String;

/// Type of timepoints
pub type Time = VarCst;

/// Type of simple int expressions (composed of at most one variable)
pub type IntTerm = aries_solver::prelude::LinTerm;

/// Type of compound integer expressions.
pub type IntExp = aries_solver::prelude::LinSum;

pub type SymAtom = IntTerm;

#[derive(Clone, Eq, PartialEq)]
pub struct StateVar {
    pub fluent: Sym,
    pub args: Vec<SymAtom>,
}

impl Debug for StateVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{:?}", self.fluent, self.args)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub enum Tag {
    TaskStart(TaskId),
    TaskEnd(TaskId),
}

/// Trait capturing the requirements of constraitns posted to a [`Sched`]
///
/// It is automatically derived for any element providing the requirements,
/// but needed for making the element dyn-compatible.
pub trait SchedConstraint: BoolExpr<SchedEncoder> + Send + Sync + Debug {}
impl<C> SchedConstraint for C where C: BoolExpr<SchedEncoder> + Send + Sync + Debug {}

type Constraint = std::sync::Arc<dyn SchedConstraint>;
pub type ConstraintID = usize;

#[derive(Clone)]
pub struct Sched {
    pub model: Model,
    pub objects: ObjectEncoding,
    pub time_scale: IntCst,
    /// temporal separation between events `(1/time_scale)`
    pub epsilon: IntCst,
    pub origin: Time,
    pub horizon: Time,
    pub makespan: Time,
    pub tasks: Tasks,
    pub effects: Effects,
    constraints: Vec<Constraint>,
}

impl Sched {
    pub fn new(time_scale: IntCst, objects: ObjectEncoding) -> Self {
        assert_eq!(time_scale, 1, "Non-integer time is not supported yet");
        let mut model = Model::new();
        let origin = Time::ZERO;
        let horizon = model.new_ivar(0, INT_CST_MAX, "horizon").into();
        let makespan = model.new_ivar(0, INT_CST_MAX, "makespan").into();
        Sched {
            model,
            objects,
            time_scale,
            epsilon: 1,
            origin,
            horizon,
            makespan,
            tasks: Default::default(),
            effects: Default::default(),
            constraints: vec![Arc::new(MakespanIsMaxTaskEnd), Arc::new(EffectCoherence)],
        }
    }

    pub fn add_task(&mut self, task: Task) -> TaskId {
        self.tasks.insert(task)
    }

    pub fn add_effect(&mut self, eff: Effect) -> EffectId {
        self.effects.add_effect(eff, &self.model)
    }

    pub fn new_timepoint(&mut self) -> Time {
        self.model.new_ivar(0, INT_CST_MAX, "_").into()
    }
    pub fn new_opt_timepoint(&mut self, scope: Lit) -> Time {
        self.model.new_optional_ivar(0, INT_CST_MAX, scope, "_").into()
    }
    pub fn add_constraint<C: SchedConstraint + 'static>(&mut self, c: C) -> ConstraintID {
        self.add_boxed_constraint(Arc::new(c))
    }
    pub fn add_boxed_constraint(&mut self, c: Arc<dyn SchedConstraint + 'static>) -> ConstraintID {
        self.constraints.push(c);
        self.constraints.len() - 1
    }

    fn encoder(self) -> SchedEncoder {
        let store = self.model.clone();
        SchedEncoder {
            sched: Arc::new(self),
            store,
            causal_links: CausalLinks::default(),
        }
    }

    pub fn encode(&self) -> Model {
        let mut encoder = self.clone().encoder();
        for c in &self.constraints {
            c.enforce(&mut encoder);
        }
        encoder.store
    }

    pub fn solve(&self) -> Option<Solution> {
        let encoding = self.encode();
        let mut solver = Solver::new(encoding);
        solver.solve(SearchLimit::None).unwrap()
    }

    pub fn explainable_solver<T: Ord + Clone>(
        &self,
        project: impl Fn(ConstraintID) -> Option<T>,
    ) -> ExplainableSolver<T> {
        ExplainableSolver::new(self, project)
    }

    pub fn print(&self, sol: &Solution) {
        println!("==== tasks ====");
        let sorted_tasks = self
            .tasks
            .iter()
            .filter(|t| sol.eval(t.presence) == Some(true))
            .sorted_by_cached_key(|t| sol.eval(t.start).unwrap());
        for t in sorted_tasks {
            println!("{}: {}", t.name, sol.eval(t.start).unwrap())
        }
        println!("==== Effects ====");
        let mut formatted_effects: Vec<String> = vec![];
        for e in self.effects.iter().sorted_by_key(|e| &e.state_var.fluent) {
            if !sol.entails(e.prez) {
                continue;
            }
            formatted_effects.push(format!(
                "{}({}): [{},{}] {} ...[{}]",
                e.state_var.fluent,
                e.state_var
                    .args
                    .iter()
                    .map(|arg| arg.evaluate(sol).unwrap())
                    .format(", "),
                e.transition_start.evaluate(sol).unwrap(),
                e.transition_end.evaluate(sol).unwrap(),
                match e.operation {
                    EffectOp::Assign(v) => format!(":= {}", v.evaluate(sol).unwrap()),
                    EffectOp::Step(v) => format!("+= {}", v.evaluate(sol).unwrap()),
                },
                e.mutex_end.evaluate(sol).unwrap(),
            ));
        }
        formatted_effects.sort();
        println!("{}", formatted_effects.iter().format("\n"));
        println!("Horizon: {}", self.horizon.evaluate(sol).unwrap())
    }
}

impl Dom for Sched {
    fn _upper_bound(&self, svar: SignedVar) -> IntCst {
        self.model._upper_bound(svar)
    }

    fn _presence(&self, var: Var) -> Lit {
        self.model._presence(var)
    }
}

impl ModelWrapper for Sched {
    type Lbl = Sym;

    fn get_model(&self) -> &aries_solver::model::Model<Self::Lbl> {
        &self.model
    }

    fn get_model_mut(&mut self) -> &mut aries_solver::model::Model<Self::Lbl> {
        &mut self.model
    }
}
