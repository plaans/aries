use anyhow::bail;
use transitive::Transitive;

use crate::parameter::Parameter;
use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::parvar::ParVar;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(Transitive)]
#[transitive(try_from(ParVar, Parameter))]
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

impl TryFrom<Parameter> for BasicParameter {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::Basic(b) => Ok(b),
            _ => bail!("unable to downcast"),
        }
    }
}