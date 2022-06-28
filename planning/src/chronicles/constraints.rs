use super::*;
use aries_core::{IntCst, Lit};
use aries_model::lang::Type;
use std::fmt::Debug;
use ConstraintType::*;

/// Generic representation of a constraint on a set of variables
#[derive(Debug, Clone)]
pub struct Constraint {
    pub variables: Vec<Atom>,
    pub tpe: ConstraintType,
    /// If set, this constraint should be reified so that is is always equal to value.
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
    pub fn eq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Neq,
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

    pub fn duration(dur: IntCst) -> Constraint {
        Constraint {
            variables: vec![],
            tpe: ConstraintType::Duration(dur),
            value: None,
        }
    }
}

impl Substitute for Constraint {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        Constraint {
            variables: self.variables.iter().map(|i| substitution.sub(*i)).collect(),
            tpe: self.tpe.clone(),
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
    Duration(IntCst),
    Or,
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
