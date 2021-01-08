mod concrete;
pub mod constraints;
pub mod preprocessing;
mod templates;

use aries_model::symbols::{SymId, SymbolTable, TypedSym};

use self::constraints::Table;
use aries_model::lang::{Atom, IAtom, Type, Variable};
use aries_model::Model;

use std::sync::Arc;

use aries_utils::input::Sym;
pub use concrete::*;

/// Represents a discrete value (symbol, integer or boolean)
pub type DiscreteValue = i32;

/// A state function is a symbol and a set of parameter and return types.
///
/// For instance `at: Robot -> Location -> Bool` is the state function with symbol `at`
/// that accepts two parameters of type `Robot` and `Location`.
///
/// Given two symbols `bob: Robot` and `kitchen: Location`, the application of the
/// *state function* `at` to these parameters:
/// `(at bob kitchen)` is a *state variable* of boolean type.
// TODO: make internals private
#[derive(Clone)]
pub struct StateFun {
    /// Symbol of this state function
    pub sym: SymId,
    /// type of the function. A vec [a, b, c] corresponds
    /// to the type `a -> b -> c` in curried notation.
    /// Hence a and b are the arguments and the last element is the return type
    pub tpe: Vec<Type>,
}
impl StateFun {
    pub fn argument_types(&self) -> &[Type] {
        &self.tpe[0..self.tpe.len() - 1]
    }
    pub fn return_type(&self) -> Type {
        *self.tpe.last().unwrap()
    }
}

#[derive(Clone)]
pub struct Ctx {
    pub model: Model,
    pub state_functions: Vec<StateFun>,
    origin: IAtom,
    horizon: IAtom,
    pub tables: Vec<Table<DiscreteValue>>,
}

impl Ctx {
    pub fn new(symbols: Arc<SymbolTable<Sym, Sym>>, state_variables: Vec<StateFun>) -> Self {
        let mut model = Model::new_with_symbols(symbols);

        let origin = IAtom::from(0);
        let horizon = model.new_ivar(0, DiscreteValue::MAX, "HORIZON").into();

        Ctx {
            model,
            state_functions: state_variables,
            origin,
            horizon,
            tables: Vec::new(),
        }
    }

    pub fn origin(&self) -> IAtom {
        self.origin
    }
    pub fn horizon(&self) -> IAtom {
        self.horizon
    }

    /// Returns the variable with a singleton domain that represents this constant symbol.
    pub fn typed_sym(&self, sym: SymId) -> TypedSym {
        TypedSym {
            sym,
            tpe: self.model.symbols.type_of(sym),
        }
    }
}

#[derive(Clone)]
pub struct ChronicleTemplate {
    pub label: Option<String>,
    pub parameters: Vec<Variable>,
    pub chronicle: Chronicle,
}
impl ChronicleTemplate {
    pub fn instantiate(
        &self,
        parameters: Vec<Variable>,
        origin: ChronicleOrigin,
    ) -> Result<ChronicleInstance, InvalidSubstitution> {
        let substitution = Sub::new(&self.parameters, &parameters)?;
        let chronicle = self.chronicle.substitute(&substitution);

        Ok(ChronicleInstance {
            parameters: parameters.iter().copied().map(Atom::from).collect(),
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

pub type TemplateID = u32;
pub type InstantiationID = u32;

// TODO: merge into chronicle origin
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Instantiation {
    pub template_id: TemplateID,
    pub instantiation_id: InstantiationID,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum ChronicleOrigin {
    /// This chronicle was present in the original problem formulation
    Original,
    /// This chronicle is an instantiation of a template chronicle
    FreeAction(Instantiation),
    /// THis chronicle was inserted to refine particular task
    Refinement {
        /// Index of the chronicle instance that contains the refined task
        instance_id: usize,
        /// Index of the refined task in the chronicle's subtasks
        task_id: usize,
    },
}

impl ChronicleOrigin {
    pub fn prefix(&self) -> String {
        match self {
            ChronicleOrigin::Original => "".to_string(),
            ChronicleOrigin::FreeAction(i) => format!("{}_{}_", i.template_id, i.instantiation_id),
            ChronicleOrigin::Refinement { instance_id, task_id } => format!("refinement_{}_{}_", instance_id, task_id),
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

#[derive(Clone)]
pub struct FiniteProblem {
    pub model: Model,
    pub origin: IAtom,
    pub horizon: IAtom,
    pub chronicles: Vec<ChronicleInstance>,
    pub tables: Vec<Table<DiscreteValue>>,
}
