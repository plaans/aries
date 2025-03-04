use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum BasicParameter {
    Bool(SharedBoolParameter),
    Int(SharedIntParameter),
}

impl Identifiable for BasicParameter {
    fn id(&self) -> &Id {
        match self {
            BasicParameter::Int(par) => par.id(),
            BasicParameter::Bool(par) => par.id(),
        }
    }
}

impl From<SharedBoolParameter> for BasicParameter {
    fn from(value: SharedBoolParameter) -> Self {
        Self::Bool(value)
    }
}

impl From<SharedIntParameter> for BasicParameter {
    fn from(value: SharedIntParameter) -> Self {
        Self::Int(value)
    }
}