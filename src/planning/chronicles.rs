use crate::collection::id_map::IdMap;
use crate::planning::ref_store::{Ref, RefStore};
use crate::planning::symbols::{Instances, SymId, SymbolTable};
use crate::planning::typesystem::TypeId;
use itertools::Itertools;
use std::cmp::Ordering;
use std::fmt::Display;

pub type TimeConstant = i64;

#[derive(Copy, Clone)]
pub struct Time<A> {
    pub reference: A,
    pub shift: TimeConstant,
}
impl<A> Time<A> {
    pub fn new(reference: A) -> Self {
        Time {
            reference,
            shift: 0i64,
        }
    }
    pub fn shifted(reference: A, delay: TimeConstant) -> Self {
        Time {
            reference,
            shift: delay,
        }
    }

    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Time<B> {
        Time {
            reference: f(&self.reference),
            shift: self.shift,
        }
    }
}

impl<A: PartialEq> PartialOrd for Time<A> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.reference == other.reference {
            Some(self.shift.cmp(&other.shift))
        } else {
            None
        }
    }
}
impl<A: PartialEq> PartialEq for Time<A> {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference && self.shift == other.shift
    }
}
impl<A: PartialEq> PartialEq<A> for Time<A> {
    fn eq(&self, other: &A) -> bool {
        &self.reference == other && self.shift == 0
    }
}

pub enum Constraint<A> {
    BeforeEq(Time<A>, Time<A>),
    Eq(A, A),
    Diff(A, A),
}

#[derive(Copy, Clone)]
pub struct Interval<A> {
    pub start: Time<A>,
    pub end: Time<A>,
}
impl<A> Interval<A> {
    pub fn new(start: Time<A>, end: Time<A>) -> Self {
        Interval { start, end }
    }
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Interval<B> {
        Interval::new(self.start.map(f), self.end.map(f))
    }
}
pub type SV<A> = Vec<A>;
pub struct Effect<A>(pub Interval<A>, pub SV<A>, pub A);
impl<A> Effect<A> {
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Effect<B> {
        Effect(self.0.map(f), self.1.iter().map(f).collect(), f(&self.2))
    }
    pub fn effective_start(&self) -> &Time<A> {
        &(self.0).end
    }
    pub fn transition_start(&self) -> &Time<A> {
        &self.0.start
    }
    pub fn variable(&self) -> &[A] {
        self.1.as_slice()
    }
    pub fn value(&self) -> &A {
        &self.2
    }
}
pub struct Condition<A>(pub Interval<A>, pub SV<A>, pub A);
impl<A> Condition<A> {
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Condition<B> {
        Condition(self.0.map(f), self.1.iter().map(f).collect(), f(&self.2))
    }
    pub fn start(&self) -> &Time<A> {
        &(self.0).end
    }
    pub fn end(&self) -> &Time<A> {
        &self.0.start
    }
    pub fn variable(&self) -> &[A] {
        self.1.as_slice()
    }
    pub fn value(&self) -> &A {
        &self.2
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum VarKind {
    Symbolic,
    Boolean,
    Integer,
    Time,
}

#[derive(Copy, Clone)]
pub struct Domain {
    kind: VarKind,
    min: isize,
    max: isize,
}
impl Domain {
    pub fn temporal(min: isize, max: isize) -> Domain {
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
                min: min as isize,
                max: max as isize,
            }
        } else {
            Domain::empty(VarKind::Symbolic)
        }
    }
}

pub type Var = usize;

struct VarMeta<A> {
    dom: Domain,
    prez: Option<A>,
    label: Option<String>,
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
    variables: RefStore<A, VarMeta<A>>,
    var_of_sym: IdMap<SymId, A>,
}

impl<T, I, A: Ref> Ctx<T, I, A> {
    pub fn new(symbols: SymbolTable<T, I>, state_variables: Vec<StateFun>) -> Self
    where
        I: Display,
    {
        let mut variables = RefStore::new();
        let mut var_of_sym = IdMap::default();
        let tautology = variables.push(VarMeta {
            dom: Domain::boolean_true(),
            prez: None,
            label: Some("true".to_string()),
        });
        let contradiction = variables.push(VarMeta {
            dom: Domain::boolean_false(),
            prez: None,
            label: Some("false".to_string()),
        });
        for sym in symbols.iter() {
            let meta = VarMeta {
                dom: Instances::singleton(sym).into(),
                prez: None, // variable represents a constant and is always present
                label: Some(format!("{}", symbols.symbol(sym))),
            };
            let var_id = variables.push(meta);
            var_of_sym.insert(sym, var_id);
        }

        let origin = variables.push(VarMeta {
            dom: Domain::temporal(0, 0),
            prez: None,
            label: Some("ORIGIN".to_string()),
        });
        let horizon = variables.push(VarMeta {
            dom: Domain::temporal(0, std::isize::MAX),
            prez: None,
            label: Some("ORIGIN".to_string()),
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
        let meta = &self.variables[variable].dom;
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
        self.variables[var].dom
    }

    pub fn presence(&self, var: A) -> Option<A> {
        self.variables[var].prez
    }
}

pub struct Chronicle<A> {
    /// human readable label to the chronicle. Not necessarily unique among chronicles
    pub prez: A,
    pub start: Time<A>,
    pub end: Time<A>,
    pub name: Vec<A>,
    pub conditions: Vec<Condition<A>>,
    pub effects: Vec<Effect<A>>,
}

impl<A> Chronicle<A> {
    pub fn map<B, F: Fn(&A) -> B>(&self, f: &F) -> Chronicle<B> {
        Chronicle {
            prez: f(&self.prez),
            start: self.start.map(f),
            end: self.end.map(f),
            name: self.name.iter().map(f).collect_vec(),
            conditions: self.conditions.iter().map(|c| c.map(f)).collect(),
            effects: self.effects.iter().map(|c| c.map(f)).collect(),
        }
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub enum Holed<A> {
    Full(A),
    Param(usize),
}

#[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub enum Type {
    Symbolic(TypeId),
    Boolean,
    Integer,
    Time,
}

pub struct ChronicleTemplate<A> {
    pub label: Option<String>,
    pub params: Vec<(Type, Option<String>)>,
    pub chronicle: Chronicle<Holed<A>>,
}
impl<A> ChronicleTemplate<A> {
    pub fn instantiate(&self, parameters: &[A]) -> ChronicleInstance<A>
    where
        A: Copy,
    {
        let chronicle = self.chronicle.map(&|hole| match hole {
            Holed::Full(a) => *a,
            Holed::Param(i) => parameters[*i],
        });
        ChronicleInstance {
            params: parameters.to_vec(),
            chronicle,
        }
    }
}

pub struct ChronicleInstance<A> {
    pub params: Vec<A>,
    pub chronicle: Chronicle<A>,
}

pub struct Problem<T, I, A: Ref> {
    pub context: Ctx<T, I, A>,
    pub templates: Vec<ChronicleTemplate<A>>,
    pub chronicles: Vec<ChronicleInstance<A>>,
}
