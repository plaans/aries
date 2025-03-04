use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(Eq, Clone, Debug)]
pub struct GenericArrayParameter<T> {
    id: Id,
    basic_parameters: Vec<T>
}

impl<T> GenericArrayParameter<T> {
    pub fn new(id: Id) -> Self {
        let basic_parameters = Vec::new();
        Self { id, basic_parameters }
    }
    
    pub fn basic_parameters(&self) -> impl Iterator<Item = &T> {
        self.basic_parameters.iter()
    }
    
    pub fn push(&mut self, basic_variable: T) {
        self.basic_parameters.push(basic_variable);
    }

    pub fn len(&self) -> usize {
        self.basic_parameters.len()
    }
}

impl<T> Identifiable for GenericArrayParameter<T> {
    fn id(&self) -> &Id {
        &self.id
    }
}

impl<T> PartialEq for GenericArrayParameter<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub type ArrayBoolParameter = GenericArrayParameter<SharedBoolParameter>;
pub type ArrayIntParameter = GenericArrayParameter<SharedIntParameter>;