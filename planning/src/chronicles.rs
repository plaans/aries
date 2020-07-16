use crate::ref_store::{Ref, RefStore};
use crate::symbols::{Instances, SymId, SymbolTable};
use crate::typesystem::TypeId;
use aries_collections::id_map::IdMap;
use itertools::Itertools;
use serde::Serialize;
use std::cmp::Ordering;
use std::fmt::Display;

pub type TimeConstant = Integer;

#[derive(Copy, Clone, Serialize)]
pub struct Time<A> {
    pub time_var: A,
    pub delay: TimeConstant,
}
impl<A> Time<A> {
    pub fn new(reference: A) -> Self {
        Time {
            time_var: reference,
            delay: 0,
        }
    }
    pub fn shifted(reference: A, delay: TimeConstant) -> Self {
        Time {
            time_var: reference,
            delay,
        }
    }

    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Time<B> {
        Time {
            time_var: f(&self.time_var),
            delay: self.delay,
        }
    }
}

impl<A: PartialEq> PartialOrd for Time<A> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.time_var == other.time_var {
            Some(self.delay.cmp(&other.delay))
        } else {
            None
        }
    }
}
impl<A: PartialEq> PartialEq for Time<A> {
    fn eq(&self, other: &Self) -> bool {
        self.time_var == other.time_var && self.delay == other.delay
    }
}
impl<A: PartialEq> PartialEq<A> for Time<A> {
    fn eq(&self, other: &A) -> bool {
        &self.time_var == other && self.delay == 0
    }
}

pub enum Constraint<A> {
    BeforeEq(Time<A>, Time<A>),
    Eq(A, A),
    Diff(A, A),
}

pub type SV<A> = Vec<A>;

#[derive(Clone, Serialize)]
pub struct Effect<A> {
    pub transition_start: Time<A>,
    pub persistence_start: Time<A>,
    pub state_var: SV<A>,
    pub value: A,
}

impl<A> Effect<A> {
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Effect<B> {
        Effect {
            transition_start: self.transition_start.map(f),
            persistence_start: self.persistence_start.map(f),
            state_var: self.state_var.iter().map(f).collect(),
            value: f(&self.value),
        }
    }
    pub fn effective_start(&self) -> &Time<A> {
        &self.persistence_start
    }
    pub fn transition_start(&self) -> &Time<A> {
        &self.transition_start
    }
    pub fn variable(&self) -> &[A] {
        self.state_var.as_slice()
    }
    pub fn value(&self) -> &A {
        &self.value
    }
}

#[derive(Clone, Serialize)]
pub struct Condition<A> {
    pub start: Time<A>,
    pub end: Time<A>,
    pub state_var: SV<A>,
    pub value: A,
}

impl<A> Condition<A> {
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Condition<B> {
        Condition {
            start: self.start.map(f),
            end: self.end.map(f),
            state_var: self.state_var.iter().map(f).collect(),
            value: f(&self.value),
        }
    }
    pub fn start(&self) -> &Time<A> {
        &self.start
    }
    pub fn end(&self) -> &Time<A> {
        &self.end
    }
    pub fn variable(&self) -> &[A] {
        self.state_var.as_slice()
    }
    pub fn value(&self) -> &A {
        &self.value
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize)]
pub enum VarKind {
    Symbolic,
    Boolean,
    Integer,
    Time,
}

pub type Integer = i32;

#[derive(Copy, Clone, Serialize)]
pub struct Domain {
    kind: VarKind,
    min: Integer,
    max: Integer,
}
impl Domain {
    pub fn symbolic(symbols: Instances) -> Domain {
        Domain::from(symbols)
    }

    pub fn temporal(min: Integer, max: Integer) -> Domain {
        Domain {
            kind: VarKind::Time,
            min,
            max,
        }
    }

    pub fn integer(min: Integer, max: Integer) -> Domain {
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
        Domain {
            kind,
            min: 0,
            max: -1,
        }
    }
}
impl From<Instances> for Domain {
    fn from(inst: Instances) -> Self {
        if let Some((min, max)) = inst.bounds() {
            let min: usize = min.into();
            let max: usize = max.into();
            Domain {
                kind: VarKind::Symbolic,
                min: min as Integer,
                max: max as Integer,
            }
        } else {
            Domain::empty(VarKind::Symbolic)
        }
    }
}

// TODO: change to a Ref
pub type Var = usize;

/// Metadata associated with a variable of type `A`
#[derive(Clone, Serialize)]
pub struct VarMeta<A> {
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
pub struct StateFun {
    /// Symbol of this state function
    pub sym: SymId,
    /// type of the function. A vec [a, b, c] corresponds
    /// to the type `a -> b -> c` in curried notation.
    /// Hence a and b are the argument and the last element is the return type
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

pub struct Ctx<T, I, A: Ref> {
    pub symbols: SymbolTable<T, I>,
    pub state_functions: Vec<StateFun>,
    tautology: A,
    contradiction: A,
    origin: A,
    horizon: A,
    pub variables: RefStore<A, VarMeta<A>>,
    var_of_sym: IdMap<SymId, A>,
}

impl<T, I, A: Ref> Ctx<T, I, A> {
    pub fn new(symbols: SymbolTable<T, I>, state_variables: Vec<StateFun>) -> Self
    where
        I: Display,
    {
        let mut variables = RefStore::new();
        let mut var_of_sym = IdMap::default();
        let contradiction = variables.push(VarMeta {
            domain: Domain::boolean_false(),
            presence: None,
            label: Some("false".to_string()),
        });
        let tautology = variables.push(VarMeta {
            domain: Domain::boolean_true(),
            presence: None,
            label: Some("true".to_string()),
        });
        for sym in symbols.iter() {
            let meta = VarMeta {
                domain: Instances::singleton(sym).into(),
                presence: None, // variable represents a constant and is always present
                label: Some(format!("{}", symbols.symbol(sym))),
            };
            let var_id = variables.push(meta);
            var_of_sym.insert(sym, var_id);
        }

        let origin = variables.push(VarMeta {
            domain: Domain::temporal(0, 0),
            presence: None,
            label: Some("ORIGIN".to_string()),
        });
        let horizon = variables.push(VarMeta {
            domain: Domain::temporal(0, Integer::MAX),
            presence: None,
            label: Some("HORIZON".to_string()),
        });
        Ctx {
            symbols,
            state_functions: state_variables,
            tautology,
            contradiction,
            origin,
            horizon,
            variables,
            var_of_sym,
        }
    }

    pub fn origin(&self) -> A {
        self.origin
    }
    pub fn horizon(&self) -> A {
        self.horizon
    }
    pub fn tautology(&self) -> A {
        self.tautology
    }
    pub fn contradiction(&self) -> A {
        self.contradiction
    }

    /// Returns the variable with a singleton domain that represents this constant symbol
    pub fn variable_of(&self, sym: SymId) -> A {
        *self
            .var_of_sym
            .get(sym)
            .expect("Symbol with no associated variable.")
    }

    pub fn sym_domain_of(&self, variable: A) -> Option<Instances> {
        let meta = &self.variables[variable].domain;
        if meta.kind == VarKind::Symbolic {
            let lb: usize = meta.min as usize;
            let ub: usize = meta.max as usize;
            Some(Instances::new(SymId::from(lb), SymId::from(ub)))
        } else {
            None // non symbolic variable
        }
    }

    pub fn sym_value_of(&self, variable: A) -> Option<SymId> {
        self.sym_domain_of(variable)
            .and_then(|x| x.into_singleton())
    }

    pub fn domain(&self, var: A) -> Domain {
        self.variables[var].domain
    }

    pub fn presence(&self, var: A) -> Option<A> {
        self.variables[var].presence
    }
}

#[derive(Clone, Serialize)]
pub struct Chronicle<A> {
    /// human readable label to the chronicle. Not necessarily unique among chronicles
    pub presence: A,
    pub start: Time<A>,
    pub end: Time<A>,
    pub name: Vec<A>,
    pub conditions: Vec<Condition<A>>,
    pub effects: Vec<Effect<A>>,
}

impl<A> Chronicle<A> {
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Chronicle<B> {
        Chronicle {
            presence: f(&self.presence),
            start: self.start.map(f),
            end: self.end.map(f),
            name: self.name.iter().map(f).collect_vec(),
            conditions: self.conditions.iter().map(|c| c.map(f)).collect(),
            effects: self.effects.iter().map(|c| c.map(f)).collect(),
        }
    }
}

/// Representation for a value that might either already know (the hole is full)
/// or unknown. When unknown the hole is empty and remains to be filled.
/// This corresponds to the `Param` variant that specifies the id of the parameter
/// from which the value should be taken.
#[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq, Serialize)]
pub enum Holed<A> {
    /// value is specified
    Full(A),
    /// value is not present yet and should be the one of the n^th parameter
    Param(usize),
}
impl<A> Holed<A> {
    pub fn fill(&self, arguments: &[A]) -> A
    where
        A: Clone,
    {
        match self {
            Holed::Full(a) => a.clone(),
            Holed::Param(i) => arguments[*i].clone(),
        }
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq, Serialize)]
pub enum Type {
    Symbolic(TypeId),
    Boolean,
    Integer,
    Time,
}

#[derive(Clone, Serialize)]
pub struct ChronicleTemplate<A> {
    pub label: Option<String>,
    pub parameters: Vec<(Type, Option<String>)>,
    pub chronicle: Chronicle<Holed<A>>,
}
impl<A> ChronicleTemplate<A> {
    pub fn instantiate(
        &self,
        parameters: &[A],
        template_id: TemplateID,
        instantiation_id: InstantiationID,
    ) -> ChronicleInstance<A>
    where
        A: Copy,
    {
        let chronicle = self.chronicle.map(&|hole| hole.fill(parameters));
        ChronicleInstance {
            parameters: parameters.to_vec(),
            origin: ChronicleOrigin::Instantiated(Instantiation {
                template_id,
                instantiation_id,
            }),
            chronicle,
        }
    }
}

pub type TemplateID = u32;
pub type InstantiationID = u32;
#[derive(Copy, Clone, Serialize)]
pub struct Instantiation {
    pub template_id: TemplateID,
    pub instantiation_id: InstantiationID,
}

#[derive(Copy, Clone, Serialize)]
pub enum ChronicleOrigin {
    /// This chronicle was present in the original problem formulation
    Original,
    /// This chronicle is an instantiation of a template chronicle
    Instantiated(Instantiation),
}

#[derive(Clone, Serialize)]
pub struct ChronicleInstance<A> {
    pub parameters: Vec<A>,
    pub origin: ChronicleOrigin,
    pub chronicle: Chronicle<A>,
}

pub struct Problem<T, I, A: Ref> {
    pub context: Ctx<T, I, A>,
    pub templates: Vec<ChronicleTemplate<A>>,
    pub chronicles: Vec<ChronicleInstance<A>>,
}

#[derive(Clone, Serialize)]
pub struct FiniteProblem<A: Ref> {
    pub variables: RefStore<A, VarMeta<A>>,
    pub origin: A,
    pub horizon: A,
    pub tautology: A,
    pub contradiction: A,
    pub chronicles: Vec<ChronicleInstance<A>>,
}
