mod concrete;
pub mod constraints;
pub mod preprocessing;
mod templates;

use aries_model::symbols::{ContiguousSymbols, SymId, SymbolTable, TypedSym};
use aries_model::types::TypeId;

use serde::{Deserialize, Serialize};

use self::constraints::Table;
use aries_model::lang::{Atom, ConversionError, IAtom, Type, Variable};
use aries_model::Model;

use std::sync::Arc;

pub use concrete::*;

pub type TimeConstant = DiscreteValue;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum VarKind {
    Symbolic,
    Boolean,
    Integer,
    Time,
}

/// Represents a discrete value (symbol, integer or boolean)
pub type DiscreteValue = i32;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub(crate) struct Domain {
    pub kind: VarKind,
    pub min: DiscreteValue,
    pub max: DiscreteValue,
}
impl Domain {
    pub fn symbolic(symbols: ContiguousSymbols) -> Domain {
        Domain::from(symbols)
    }

    pub fn temporal(min: DiscreteValue, max: DiscreteValue) -> Domain {
        Domain {
            kind: VarKind::Time,
            min,
            max,
        }
    }

    pub fn integer(min: DiscreteValue, max: DiscreteValue) -> Domain {
        Domain {
            kind: VarKind::Time,
            min,
            max,
        }
    }

    pub fn boolean() -> Domain {
        Domain {
            kind: VarKind::Boolean,
            min: 0,
            max: 1,
        }
    }
    pub fn boolean_true() -> Domain {
        Domain {
            kind: VarKind::Boolean,
            min: 1,
            max: 1,
        }
    }
    pub fn boolean_false() -> Domain {
        Domain {
            kind: VarKind::Boolean,
            min: 0,
            max: 0,
        }
    }
    pub fn empty(kind: VarKind) -> Domain {
        Domain { kind, min: 0, max: -1 }
    }

    pub fn contains(&self, sym: SymId) -> bool {
        if self.kind != VarKind::Symbolic {
            return false;
        }
        let id = (usize::from(sym)) as DiscreteValue;
        self.min <= id && id <= self.max
    }

    pub fn as_singleton(&self) -> Option<DiscreteValue> {
        if self.size() == 1 {
            Some(self.min)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.max < self.min
    }

    pub fn intersects(&self, other: &Domain) -> bool {
        self.kind == other.kind
            && !self.is_empty()
            && !other.is_empty()
            && self.max >= other.min
            && other.max >= self.min
    }

    pub fn size(&self) -> u32 {
        (self.max - self.min + 1) as u32
    }
}
impl From<ContiguousSymbols> for Domain {
    fn from(inst: ContiguousSymbols) -> Self {
        if let Some((min, max)) = inst.bounds() {
            let min: usize = min.into();
            let max: usize = max.into();
            Domain {
                kind: VarKind::Symbolic,
                min: min as DiscreteValue,
                max: max as DiscreteValue,
            }
        } else {
            Domain::empty(VarKind::Symbolic)
        }
    }
}

// TODO: change to a Ref
pub(crate) type Var = usize;

/// Metadata associated with a variable of type `A`
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct VarMeta<A> {
    pub domain: Domain,
    pub presence: Option<A>,
    pub label: Option<String>,
}

impl<A> VarMeta<A> {
    pub fn new(domain: Domain, presence: Option<A>, label: Option<String>) -> Self {
        VarMeta {
            domain,
            presence,
            label,
        }
    }
}

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
    pub fn new(symbols: Arc<SymbolTable<String, String>>, state_variables: Vec<StateFun>) -> Self {
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
        template_id: TemplateID,
        instantiation_id: InstantiationID,
    ) -> Result<ChronicleInstance, InvalidSubstitution> {
        let substitution = Sub::new(&self.parameters, &parameters)?;
        let chronicle = self.chronicle.substitute(&substitution);

        Ok(ChronicleInstance {
            parameters: parameters.iter().copied().map(Atom::from).collect(),
            origin: ChronicleOrigin::Instantiated(Instantiation {
                template_id,
                instantiation_id,
            }),
            chronicle,
        })
    }
}

pub type TemplateID = u32;
pub type InstantiationID = u32;
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Instantiation {
    pub template_id: TemplateID,
    pub instantiation_id: InstantiationID,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ChronicleOrigin {
    /// This chronicle was present in the original problem formulation
    Original,
    /// This chronicle is an instantiation of a template chronicle
    Instantiated(Instantiation),
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
