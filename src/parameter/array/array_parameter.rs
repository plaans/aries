use crate::parameter::SharedArrayBoolParameter;
use crate::parameter::SharedArrayIntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ArrayParameter {
    Bool(SharedArrayBoolParameter),
    Int(SharedArrayIntParameter),
}

impl Identifiable for ArrayParameter {
    fn id(&self) -> &Id {
        match self {
            ArrayParameter::Bool(b) => b.id(),
            ArrayParameter::Int(i) => i.id(),
        }
    }
}