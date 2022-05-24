use super::*;
use aries_model::lang::Type;

/// Generic representation of a constraint on a set of variables
#[derive(Debug, Clone)]
pub struct Constraint {
    pub variables: Vec<Atom>,
    pub tpe: ConstraintType,
}
use aries_core::IntCst;
use ConstraintType::*;

impl Constraint {
    pub fn atom(a: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into()],
            tpe: Or,
        }
    }

    pub fn lt(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Lt,
        }
    }
    pub fn eq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Neq,
        }
    }
    pub fn neq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Neq,
        }
    }

    pub fn duration(dur: IntCst) -> Constraint {
        Constraint {
            variables: vec![],
            tpe: ConstraintType::Duration(dur),
        }
    }
}

impl Substitute for Constraint {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        Constraint {
            variables: self.variables.iter().map(|i| substitution.sub(*i)).collect(),
            tpe: self.tpe,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ConstraintType {
    /// Variables should take a value as one of the tuples in the corresponding table.
    InTable {
        table_id: u32,
    },
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
    /// Number of elements in the tuple
    line_size: usize,
    /// Type of the values in the tuples (length = line_size)
    types: Vec<Type>,
    /// linear representation of a matrix (each line occurs right after the previous one)
    inner: Vec<E>,
}

impl<E: Clone> Table<E> {
    pub fn new(types: Vec<Type>) -> Table<E> {
        Table {
            line_size: types.len(),
            types,
            inner: Vec::new(),
        }
    }

    pub fn push(&mut self, line: &[E]) {
        assert!(line.len() == self.line_size);
        self.inner.extend_from_slice(line);
    }

    pub fn lines(&self) -> impl Iterator<Item = &[E]> {
        self.inner.chunks(self.line_size)
    }
}
