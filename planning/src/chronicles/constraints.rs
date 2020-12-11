use super::*;

/// Generic representation of a constraint on a set of variables
#[derive(Debug, Clone)]
pub struct Constraint {
    pub variables: Vec<Atom>,
    pub tpe: ConstraintType,
}

#[derive(Copy, Clone, Debug)]
pub enum ConstraintType {
    /// Variables should take a value as one of the tuples in the corresponding table.
    InTable { table_id: u32 },
}

/// A set of tuples, representing the allowed values in a table constraint.
#[derive(Clone, Serialize, Deserialize)]
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
