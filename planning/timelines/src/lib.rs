pub mod boxes;
pub mod constraints;
mod effects;
pub mod encoder;
pub mod explain;
pub mod rational;
pub mod symbols;
pub mod tasks;
pub mod transitions;

use aries::core::state::Evaluable;
use aries::core::views::Dom;
use constraints::*;
use core::fmt::Debug;
use core::hash::{Hash, Hasher};
use std::sync::Arc;
use std::collections::HashMap;

use aries::core::INT_CST_MAX;
pub use aries::core::IntCst;
use aries::model::lang::*;
use aries::prelude::*;
use aries::solver::Solver;
use idmap::{DirectIdMap, DirectIdSet};
use itertools::Itertools;

pub type Model = aries::model::Model<Sym>;
pub use crate::effects::*;
use crate::encoder::SchedEncoder;
use crate::explain::ExplainableSolver;
use crate::symbols::ObjectEncoding;
pub use crate::tasks::*;
use crate::transitions::{TransitionId, Transitions};

pub type Sym = String;

/// Type of timepoints
pub type Time = IAtom;

/// Type of simple int expressions (composed of at most one variable)
pub type IntTerm = aries::prelude::LinTerm;

/// Type of compound integer expressions.
pub type IntExp = aries::prelude::LinSum;

pub type SymAtom = IntTerm;

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
    conditions: DirectIdMap<ConstraintID, HasValueAt>,
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
            conditions: Default::default(),
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
    pub fn add_condition(&mut self, c: HasValueAt) -> ConstraintID {
        let c_cloned = HasValueAt {
            state_var: c.state_var.clone(),
            value: c.value,
            timepoint: c.timepoint,
            prez: c.prez,
            source: c.source,
        };
        let cid = self.add_constraint(c_cloned);
        self.conditions.insert(cid, c);
        cid
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
        }
    }

    pub fn encode(&self) -> Model {
        let mut encoder = self.clone().encoder();
        for c in &self.constraints {
            c.enforce(&mut encoder);
        }
        encoder.store
    }
    pub fn gather_transitions(&self) -> Transitions {
        let mut effects: HashMap<Option<TaskId>, Vec<(EffectId, &Effect)>> = HashMap::new();
        let mut conditions: HashMap<Option<TaskId>, Vec<(ConstraintID, &HasValueAt)>> = HashMap::new();

        for (id, e) in self.effects.iter().enumerate() {
            effects.entry(e.source).and_modify(|v| v.push((id, e))).or_insert(vec![(id, e)]);
        }
        for (id, c) in self.conditions.iter() {
            conditions.entry(c.source).and_modify(|v| v.push((id, c))).or_insert(vec![(id, c)]);
        }

        let mut lifted = vec![];
        let mut e_in_condeff = DirectIdSet::new();
        let mut c_in_condeff = DirectIdSet::new();

        for (&e_src, es) in &effects {
            for &(e_id, e) in es {
                if let Some(cs) = conditions.get(&e_src) {
                    for &(c_id, c) in cs {
                        if e.state_var == c.state_var {
                            lifted.push(TransitionId::CondEff(c_id, e_id));
                            c_in_condeff.insert(c_id);
                            e_in_condeff.insert(e_id);
                        }
                    }
                }
            }
            for &(e_id, _) in es {
                if !e_in_condeff.contains(e_id) {
                    lifted.push(TransitionId::Eff(e_id))
                }
            }
        };
        for cs in conditions.values() {
            for &(c_id, _) in cs {
                if !c_in_condeff.contains(c_id) {
                    lifted.push(TransitionId::Cond(c_id))
                }
            }
        }

        Transitions::from_lifted(lifted)
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
        for e in self.effects.iter().sorted_by_key(|e| &e.state_var.fluent) {
            if !sol.entails(e.prez) {
                println!("{:?}", e);
                continue;
            }
            println!(
                "{}: [{},{}] {} ...[{}]",
                e.state_var.fluent,
                e.transition_start.evaluate(sol).unwrap(),
                e.transition_end.evaluate(sol).unwrap(),
                match e.operation {
                    EffectOp::Assign(v) => format!(":= {}", v.evaluate(sol).unwrap()),
                    EffectOp::Step(v) => format!("+= {}", v.evaluate(sol).unwrap()),
                },
                e.mutex_end.evaluate(sol).unwrap(),
            );
        }
        println!("Horizon: {}", self.horizon.evaluate(sol).unwrap())
    }
}

impl Dom for Sched {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.model.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.model.presence(var)
    }
}
