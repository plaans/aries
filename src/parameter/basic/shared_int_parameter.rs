use std::ops::Deref;
use std::rc::Rc;

use anyhow::bail;
use transitive::Transitive;

use crate::parameter::BasicParameter;
use crate::parameter::IntParameter;
use crate::parameter::Parameter;
use crate::parvar::ParVar;

#[derive(Transitive)]
#[transitive(try_from(Parameter, BasicParameter))]
#[transitive(try_from(ParVar, Parameter, BasicParameter))]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct SharedIntParameter(Rc<IntParameter>);

impl Deref for SharedIntParameter {
    type Target = IntParameter;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<IntParameter> for SharedIntParameter {
    fn from(value: IntParameter) -> Self {
        Self { 0: Rc::new(value) }
    }
}

impl TryFrom<BasicParameter> for SharedIntParameter {
    type Error = anyhow::Error;

    fn try_from(value: BasicParameter) -> Result<Self, Self::Error> {
        match value {
            BasicParameter::Int(p) => Ok(p),
            _ => bail!("unable to downcast"),
        }
    }
}