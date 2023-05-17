use super::*;
use aries::core::Lit;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::Type;
use std::fmt::Debug;
use ConstraintType::*;

/// Generic representation of a constraint on a set of variables
#[derive(Debug, Clone)]
pub struct Constraint {
    pub variables: Vec<Atom>,
    pub tpe: ConstraintType,
    /// If set, this constraint should be reified so that it is always equal to value.
    pub value: Option<Lit>,
}

impl Constraint {
    pub fn atom(a: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into()],
            tpe: Or,
            value: None,
        }
    }

    pub fn lt(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Lt,
            value: None,
        }
    }
    pub fn fleq(a: impl Into<FAtom>, b: impl Into<FAtom>) -> Constraint {
        let a = a.into();
        let b = b.into();
        Constraint::lt(a, b + FAtom::EPSILON)
    }
    pub fn reified_lt(a: impl Into<Atom>, b: impl Into<Atom>, constraint_value: Lit) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Lt,
            value: Some(constraint_value),
        }
    }
    pub fn eq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Eq,
            value: None,
        }
    }
    pub fn reified_eq(a: impl Into<Atom>, b: impl Into<Atom>, constraint_value: Lit) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Eq,
            value: Some(constraint_value),
        }
    }
    pub fn neq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Neq,
            value: None,
        }
    }

    pub fn duration(dur: Duration) -> Constraint {
        Constraint {
            variables: vec![],
            tpe: ConstraintType::Duration(dur),
            value: None,
        }
    }

    // /// Returns true if the
    // pub fn is_tautological(self) -> bool {
    //     match self.tpe {
    //         ConstraintType::Lt => {
    //             if self.variables.len() == 2 && let Some(a) = self.variables[0]
    //         }
    //     }
    // }
}

impl Substitute for Constraint {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        Constraint {
            variables: self.variables.iter().map(|i| substitution.sub(*i)).collect(),
            tpe: self.tpe.substitute(substitution),
            value: self.value.map(|v| substitution.sub_lit(v)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConstraintType {
    /// Variables should take a value as one of the tuples in the corresponding table.
    InTable(Arc<Table<DiscreteValue>>),
    Lt,
    Eq,
    Neq,
    Duration(Duration),
    Or,
}

impl Substitute for ConstraintType {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        match self {
            Duration(Duration::Fixed(f)) => ConstraintType::Duration(Duration::Fixed(substitution.sub_linear_sum(f))),
            Duration(Duration::Bounded { lb, ub }) => ConstraintType::Duration(Duration::Bounded {
                lb: substitution.sub_linear_sum(lb),
                ub: substitution.sub_linear_sum(ub),
            }),
            InTable(_) | Lt | Eq | Neq | Or => self.clone(), // no variables in those variants
        }
    }
}

/// A set of tuples, representing the allowed values in a table constraint.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Table<E> {
    /// A human readable name to describe the table's content (typically the name of the property)
    pub name: String,
    /// Number of elements in the tuple
    line_size: usize,
    /// Type of the values in the tuples (length = line_size)
    types: Vec<Type>,
    /// linear representation of a matrix (each line occurs right after the previous one)
    inner: Vec<E>,
}

impl<E> Debug for Table<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "table({})", self.name)
    }
}

impl<E: Clone> Table<E> {
    pub fn new(name: String, types: Vec<Type>) -> Table<E> {
        Table {
            name,
            line_size: types.len(),
            types,
            inner: Vec::new(),
        }
    }

    pub fn push(&mut self, line: &[E]) {
        assert_eq!(line.len(), self.line_size);
        self.inner.extend_from_slice(line);
    }

    pub fn lines(&self) -> impl Iterator<Item = &[E]> {
        self.inner.chunks(self.line_size)
    }
}

/// Constraint that restricts the allowed durations of a chronicle
#[derive(Clone, Debug)]
pub enum Duration {
    /// The chronicle has a fixed the duration.
    Fixed(LinearSum),
    /// The duration must be between the lower and the upper bound (inclusive)
    Bounded { lb: LinearSum, ub: LinearSum },
}
