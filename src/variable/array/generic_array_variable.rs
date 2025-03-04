use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;

#[derive(Eq, Clone, Debug)]
pub struct GenericArrayVariable<T> {
    id: Id,
    basic_variables: Vec<T>
}

impl<T> GenericArrayVariable<T> {
    pub fn new(id: Id) -> Self {
        let basic_variables = Vec::new();
        Self { id, basic_variables }
    }
    
    pub fn basic_variables(&self) -> impl Iterator<Item = &T> {
        self.basic_variables.iter()
    }
    
    pub fn push(&mut self, basic_variable: T) {
        self.basic_variables.push(basic_variable);
    }

    pub fn len(&self) -> usize {
        self.basic_variables.len()
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

pub type ArrayBoolVariable = GenericArrayVariable<SharedBoolVariable>;
pub type ArrayIntVariable = GenericArrayVariable<SharedIntVariable>;