use std::rc::Rc;

use transitive::Transitive;

use crate::parameter::BoolArrayParameter;
use crate::parameter::BoolParameter;
use crate::parameter::IntArrayParameter;
use crate::parameter::IntParameter;
use crate::parvar::ParVar;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(Transitive)]
#[transitive(from(BoolParameter, Rc<BoolParameter>))]
#[transitive(from(IntParameter, Rc<IntParameter>))]
#[transitive(from(BoolArrayParameter, Rc<BoolArrayParameter>))]
#[transitive(from(IntArrayParameter, Rc<IntArrayParameter>))]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Parameter {
    BoolParameter(Rc<BoolParameter>),
    IntParameter(Rc<IntParameter>),
    BoolArrayParameter(Rc<BoolArrayParameter>),
    IntArrayParameter(Rc<IntArrayParameter>),
}

impl Identifiable for Parameter {
    fn id(&self) -> &Id {
        match self {
            Parameter::BoolParameter(p) => p.id(),
            Parameter::IntParameter(p) => p.id(),
            Parameter::BoolArrayParameter(p) => p.id(),
            Parameter::IntArrayParameter(p) => p.id(),
        }
    }
}

impl From<Rc<BoolParameter>> for Parameter {
    fn from(value: Rc<BoolParameter>) -> Self {
        Self::BoolParameter(value)
    }
}

impl From<Rc<IntParameter>> for Parameter {
    fn from(value: Rc<IntParameter>) -> Self {
        Self::IntParameter(value)
    }
}

impl From<Rc<BoolArrayParameter>> for Parameter {
    fn from(value: Rc<BoolArrayParameter>) -> Self {
        Self::BoolArrayParameter(value)
    }
}

impl From<Rc<IntArrayParameter>> for Parameter {
    fn from(value: Rc<IntArrayParameter>) -> Self {
        Self::IntArrayParameter(value)
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

impl TryFrom<Parameter> for Rc<BoolParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::BoolParameter(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Parameter> for Rc<IntParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::IntParameter(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Parameter> for Rc<BoolArrayParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::BoolArrayParameter(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Parameter> for Rc<IntArrayParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::IntArrayParameter(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}
