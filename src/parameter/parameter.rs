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
    Bool(Rc<BoolParameter>),
    Int(Rc<IntParameter>),
    BoolArray(Rc<BoolArrayParameter>),
    IntArray(Rc<IntArrayParameter>),
}

impl Identifiable for Parameter {
    fn id(&self) -> &Id {
        match self {
            Parameter::Bool(p) => p.id(),
            Parameter::Int(p) => p.id(),
            Parameter::BoolArray(p) => p.id(),
            Parameter::IntArray(p) => p.id(),
        }
    }
}

impl From<Rc<BoolParameter>> for Parameter {
    fn from(value: Rc<BoolParameter>) -> Self {
        Self::Bool(value)
    }
}

impl From<Rc<IntParameter>> for Parameter {
    fn from(value: Rc<IntParameter>) -> Self {
        Self::Int(value)
    }
}

impl From<Rc<BoolArrayParameter>> for Parameter {
    fn from(value: Rc<BoolArrayParameter>) -> Self {
        Self::BoolArray(value)
    }
}

impl From<Rc<IntArrayParameter>> for Parameter {
    fn from(value: Rc<IntArrayParameter>) -> Self {
        Self::IntArray(value)
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
            Parameter::Bool(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Parameter> for Rc<IntParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::Int(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Parameter> for Rc<BoolArrayParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::BoolArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Parameter> for Rc<IntArrayParameter> {
    type Error = anyhow::Error;

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::IntArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}
