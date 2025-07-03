pub mod analysis;
mod concrete;
pub mod constraints;
pub mod plan;
pub mod preprocessing;
pub mod printer;

pub use concrete::*;

use self::constraints::Table;
use crate::chronicles::preprocessing::action_rolling::RollCompilation;
use aries::core::{IntCst, INT_CST_MAX};
use aries::model::extensions::Shaped;
use aries::model::lang::{Atom, FAtom, IAtom, Type, Variable};
use aries::model::symbols::{SymId, SymbolTable, TypedSym};
use aries::model::Model;
use aries::utils::input::Sym;
use env_param::EnvParam;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Time being represented as a fixed point numeral, this is the denominator of any time numeral.
/// Having a time scale 100, will allow a resolution of `0.01` for time values.
pub static TIME_SCALE: EnvParam<IntCst> = EnvParam::new("ARIES_LCP_TIME_SCALE", "10");

/// A fluent is a state function is a symbol and a set of parameter and return types.
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
    pub name: Sym,
    /// Symbol of this fluent
    pub sym: SymId,
    /// Signature of the function. A vec [a, b, c] corresponds
    /// to the type `a -> b -> c` in curried notation.
    /// Hence `a` and `b` are the arguments and the last element `c` is the return type
    pub signature: Vec<Type>,
}
impl Fluent {
    pub fn argument_types(&self) -> &[Type] {
        &self.signature[0..self.signature.len() - 1]
    }
    pub fn return_type(&self) -> Type {
        *self.signature.last().unwrap()
    }
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

#[derive(Clone)]
pub struct Ctx {
    pub model: Model<VarLabel>,
    pub fluents: Vec<Arc<Fluent>>,
    origin: FAtom,
    horizon: FAtom,
    makespan_ub: FAtom,
    /// A reification of the final value of a state variable to optimize, if any.
    metric_final_value: Option<IAtom>,
}

impl Ctx {
    pub fn new(symbols: Arc<SymbolTable>, fluents: Vec<Fluent>) -> Self {
        let mut model = Model::new_with_symbols(symbols);

        let origin = FAtom::new(IAtom::ZERO, TIME_SCALE.get());
        let horizon = model
            .new_fvar(0, INT_CST_MAX, TIME_SCALE.get(), Container::Base / VarType::Horizon)
            .into();
        let makespan_ub = model
            .new_fvar(0, INT_CST_MAX, TIME_SCALE.get(), Container::Base / VarType::Makespan)
            .into();

        Ctx {
            model,
            fluents: fluents.into_iter().map(Arc::new).collect(),
            origin,
            horizon,
            makespan_ub,
            metric_final_value: None,
        }
    }

    pub fn origin(&self) -> FAtom {
        self.origin
    }
    pub fn horizon(&self) -> FAtom {
        self.horizon
    }
    pub fn makespan_ub(&self) -> FAtom {
        self.makespan_ub
    }

    pub fn metric_final_value(&self) -> Option<IAtom> {
        self.metric_final_value
    }
    pub fn set_metric_final_value(&mut self, value: IAtom) {
        debug_assert!(
            self.metric_final_value.is_none(),
            "Metric final value should only be set once"
        );
        self.metric_final_value = Some(value);
    }

    /// Returns the variable with a singleton domain that represents this constant symbol.
    pub fn typed_sym(&self, sym: SymId) -> TypedSym {
        TypedSym {
            sym,
            tpe: self.model.get_type_of(sym),
        }
    }

    pub fn get_fluent(&self, name: SymId) -> Option<&Arc<Fluent>> {
        self.fluents.iter().find(|&fluent| fluent.sym == name)
    }
}

#[derive(Clone)]
pub enum ChronicleLabel {
    /// Denotes an action with the given name
    Action(String),
    /// Rolled up version of the action
    RolledAction(String, Arc<RollCompilation>),
}

impl Debug for ChronicleLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl Display for ChronicleLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChronicleLabel::Action(name) => write!(f, "{name}"),
            ChronicleLabel::RolledAction(name, _) => write!(f, "{name}+"),
        }
    }
}

#[derive(Clone)]
pub struct ChronicleTemplate {
    pub label: ChronicleLabel,
    pub parameters: Vec<Variable>,
    pub chronicle: Chronicle,
}
impl ChronicleTemplate {
    pub fn instantiate(
        &self,
        substitution: Sub,
        origin: ChronicleOrigin,
    ) -> Result<ChronicleInstance, InvalidSubstitution> {
        debug_assert!(self.parameters.iter().all(|v| substitution.contains(*v)));
        let chronicle = self.chronicle.substitute(&substitution);
        let parameters = self
            .parameters
            .iter()
            .map(|v| substitution.sub(Atom::from(*v)))
            .collect();
        Ok(ChronicleInstance {
            parameters,
            origin,
            chronicle,
        })
    }

    /// Returns the index of this variables in the parameters of this template,
    /// or None if it is not a parameter.
    pub fn parameter_index(&self, x: impl Into<Variable>) -> Option<usize> {
        let x = x.into();
        self.parameters.iter().position(|p| p == &x)
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum ChronicleOrigin {
    /// This chronicle was present in the original problem formulation.
    /// THis is typically the case of the chronicle containing the initial state and goals.
    Original,
    /// This chronicle is an instantiation of a template chronicle
    FreeAction {
        /// Index of the chronicle template from which this chronicle was instantiated in the template list
        template_id: usize,
        /// Number of instances of this template that were previously instantiated.
        generation_id: usize,
    },
    /// This chronicle was inserted to refine one of the following tasks. All tasks must be mutually exclusive
    Refinement { refined: Vec<TaskId>, template_id: usize },
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TaskId {
    /// Index of the chronicle instance that contains the refined task
    pub instance_id: usize,
    /// Index of the refined task in the chronicle's subtasks
    pub task_id: usize,
}

impl ChronicleOrigin {
    pub fn prefix(&self) -> String {
        match self {
            ChronicleOrigin::Original => "".to_string(),
            ChronicleOrigin::FreeAction {
                template_id,
                generation_id: instantiation_id,
            } => format!("{template_id}_{instantiation_id}_"),
            ChronicleOrigin::Refinement { .. } => "refinement_".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct ChronicleInstance {
    pub parameters: Vec<Atom>,
    pub origin: ChronicleOrigin,
    pub chronicle: concrete::Chronicle,
}

#[derive(Clone)]
pub struct Problem {
    pub context: Ctx,
    pub templates: Vec<ChronicleTemplate>,
    pub chronicles: Vec<ChronicleInstance>,
}

/// Label of a variable in the encoding of a planning problem.
///  It is composed of:
/// - a container (typically the chronicle in which the variable appears)
/// - the type of the variable
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct VarLabel(pub Container, pub VarType);

impl VarLabel {
    pub fn on_instance(&self, instance_id: usize) -> Self {
        VarLabel(Container::Instance(instance_id), self.1.clone())
    }
}

impl std::fmt::Debug for VarLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}::{:?}", self.0, self.1)
    }
}

impl std::fmt::Display for VarLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Container {
    Base,
    Template(usize),
    Instance(usize),
}

impl Container {
    pub fn var(self, tpe: VarType) -> VarLabel {
        VarLabel(self, tpe)
    }
}

impl std::ops::Div<VarType> for Container {
    type Output = VarLabel;

    fn div(self, rhs: VarType) -> Self::Output {
        self.var(rhs)
    }
}

/// Label of a variable
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum VarType {
    Horizon,
    Makespan,
    Presence,
    ChronicleStart,
    ChronicleEnd,
    EffectEnd,
    /// Start time of the i-th task
    TaskStart(u32),
    /// End time of the i-th task
    TaskEnd(u32),
    /// A chronicle parameter, with the name of the parameter
    Parameter(String),
    Reification,
    Cost,
}

#[derive(Clone)]
pub struct FiniteProblem {
    pub model: Model<VarLabel>,
    pub origin: Time,
    /// Timepoint after which the state is not allowed to change
    pub horizon: Time,
    /// Timepoint after which no action is allowed
    pub makespan_ub: Time,
    pub chronicles: Vec<ChronicleInstance>,
    pub meta: Arc<analysis::Metadata>,
}
