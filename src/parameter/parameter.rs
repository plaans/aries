use crate::parameter::ArrayParameter;
use crate::parameter::BasicParameter;
use crate::parameter::BoolParameter;
use crate::parameter::IntParameter;
use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::parvar::ParVar;
use crate::traits::Identifiable;
use crate::transitive_conversion;
use crate::types::Id;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Parameter {
    Basic(BasicParameter),
    Array(ArrayParameter),
}

impl Identifiable for Parameter {
    fn id(&self) -> &Id {
        match self {
            Parameter::Basic(b) => b.id(),
            Parameter::Array(a) => a.id(),
        }
    }
}

impl From<BasicParameter> for Parameter {
    fn from(value: BasicParameter) -> Self {
        Self::Basic(value)
    }
}

impl From<ArrayParameter> for Parameter {
    fn from(value: ArrayParameter) -> Self {
        Self::Array(value)
    }
}

impl TryFrom<ParVar> for Parameter {
    type Error = anyhow::Error;

    fn try_from(value: ParVar) -> Result<Self, Self::Error> {
        match value {
            ParVar::Par(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

transitive_conversion!(Parameter, BasicParameter, SharedBoolParameter);
transitive_conversion!(Parameter, SharedBoolParameter, BoolParameter);
transitive_conversion!(Parameter, BasicParameter, SharedIntParameter);
transitive_conversion!(Parameter, SharedIntParameter, IntParameter);