pub mod analysis;
mod concrete;
pub mod constraints;
pub mod plan;
pub mod preprocessing;
pub mod printer;

use aries_solver::core::views::Term;
pub use concrete::*;

use self::constraints::Table;
use crate::chronicles::preprocessing::action_rolling::RollCompilation;
use crate::legacy::input::Sym;
use crate::legacy::*;
use aries_env_param::EnvParam;
use aries_solver::core::{IntCst, INT_CST_MAX};
use aries_solver::model::Model;
use aries_solver::prelude::*;
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
    metric_final_value: Option<VarCst>,
    pub symbols: Arc<SymbolTable>,
}

impl Ctx {
    pub fn new(symbols: Arc<SymbolTable>, fluents: Vec<Fluent>) -> Self {
        let mut model = Model::new();

        let origin = FAtom::new(VarCst::ZERO, TIME_SCALE.get());
        let horizon = model.new_ivar(0, INT_CST_MAX, Container::Base / VarType::Horizon);
        let horizon = FVar::new(horizon, TIME_SCALE.get()).into();
        let makespan_ub = model.new_ivar(0, INT_CST_MAX, Container::Base / VarType::Makespan);
        let makespan_ub = FVar::new(makespan_ub, TIME_SCALE.get()).into();

        Ctx {
            model,
            fluents: fluents.into_iter().map(Arc::new).collect(),
            origin,
            horizon,
            makespan_ub,
            metric_final_value: None,
            symbols,
        }
    }

    pub fn new_fvar(&mut self, num_lb: IntCst, num_ub: IntCst, denom: IntCst, label: impl Into<VarLabel>) -> FVar {
        let ivar = self.model.new_ivar(num_lb, num_ub, label);
        FVar::new(ivar, denom)
    }
    pub fn new_optional_fvar(
        &mut self,
        num_lb: IntCst,
        num_ub: IntCst,
        denom: IntCst,
        presence: Lit,
        label: impl Into<VarLabel>,
    ) -> FVar {
        let ivar = self.model.new_optional_ivar(num_lb, num_ub, presence, label);
        FVar::new(ivar, denom)
    }
    pub fn new_sym_var(&mut self, tpe: TypeId, label: impl Into<VarLabel>) -> SVar {
        self.create_sym_var(tpe, None, label)
    }

    pub fn new_optional_sym_var(&mut self, tpe: TypeId, presence: impl Into<Lit>, label: impl Into<VarLabel>) -> SVar {
        self.create_sym_var(tpe, Some(presence.into()), label)
    }

    fn create_sym_var(&mut self, tpe: TypeId, presence: Option<Lit>, label: impl Into<VarLabel>) -> SVar {
        let instances = self.symbols.instances_of_type(tpe);
        let presence = presence.unwrap_or(Lit::TRUE);
        // get the lower and upper bounds, defaulting to an empty interval if there are no values.
        let (lb, ub) = instances
            .bounds()
            .map(|(lb, ub)| (usize::from(lb) as IntCst, usize::from(ub) as IntCst))
            .unwrap_or((0, -1));
        let dvar = self.model.new_optional_ivar(lb, ub, presence, label).variable();
        SVar::new(dvar, tpe)
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

    pub fn metric_final_value(&self) -> Option<VarCst> {
        self.metric_final_value
    }
    pub fn set_metric_final_value(&mut self, value: VarCst) {
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
            tpe: self.get_type_of(sym),
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
    pub symbols: Arc<SymbolTable>,
    pub model: Model<VarLabel>,
    pub origin: Time,
    /// Timepoint after which the state is not allowed to change
    pub horizon: Time,
    /// Timepoint after which no action is allowed
    pub makespan_ub: Time,
    pub chronicles: Vec<ChronicleInstance>,
    pub meta: Arc<analysis::Metadata>,
}
