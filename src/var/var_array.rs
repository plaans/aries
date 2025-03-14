use std::rc::Rc;

use crate::traits::Flatzinc;
use crate::traits::Name;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Eq, Debug)]
pub struct GenArrayVariable<T> {
    variables: Vec<T>,
    name: Option<String>,
}

impl<T> GenArrayVariable<T> {
    pub fn new(variables: Vec<T>, name: Option<String>) -> Self {
        Self { variables, name }
    }
    
    pub fn variables(&self) -> impl Iterator<Item = &T> {
        self.variables.iter()
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }
}

impl<T> Name for GenArrayVariable<T> {
    fn name(&self) -> &Option<String> {
        &self.name
    }
}

impl<T: Flatzinc> Flatzinc for GenArrayVariable<T> {
    fn fzn(&self) -> String {
        self.variables.fzn()
    }
}

impl<T: PartialEq> PartialEq for GenArrayVariable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.variables == other.variables
    }
}

pub type VarBoolArray = GenArrayVariable<Rc<VarBool>>;
pub type VarIntArray = GenArrayVariable<Rc<VarInt>>;