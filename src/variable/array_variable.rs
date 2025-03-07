use std::rc::Rc;

use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;

#[derive(Eq, Clone, Debug)]
pub struct GenericArrayVariable<T> {
    id: Id,
    variables: Vec<T>
}

impl<T> GenericArrayVariable<T> {
    pub fn new(id: Id, variables: Vec<T>) -> Self {
        Self { id, variables }
    }
    
    pub fn variables(&self) -> impl Iterator<Item = &T> {
        self.variables.iter()
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }
}

impl<T> Identifiable for GenericArrayVariable<T> {
    fn id(&self) -> &Id {
        &self.id
    }
}

impl<T> PartialEq for GenericArrayVariable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub type BoolArrayVariable = GenericArrayVariable<Rc<BoolVariable>>;
pub type IntArrayVariable = GenericArrayVariable<Rc<IntVariable>>;