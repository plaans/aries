use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum Parameter {
    Bool(SharedBoolParameter),
    Int(SharedIntParameter),
}

impl Identifiable for Parameter {
    fn id(&self) -> &Id {
        match self {
            Parameter::Int(par) => par.id(),
            Parameter::Bool(par) => par.id(),
        }
    }
}

impl From<SharedBoolParameter> for Parameter {
    fn from(value: SharedBoolParameter) -> Self {
        Self::Bool(value)
    }
}

impl From<SharedIntParameter> for Parameter {
    fn from(value: SharedIntParameter) -> Self {
        Self::Int(value)
    }
}