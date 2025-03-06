use std::ops::Deref;
use std::rc::Rc;

use anyhow::bail;
use transitive::Transitive;

use crate::parameter::BasicParameter;
use crate::parameter::BoolParameter;
use crate::parameter::Parameter;
use crate::parvar::ParVar;

#[derive(Transitive)]
#[transitive(try_from(Parameter, BasicParameter))]
#[transitive(try_from(ParVar, Parameter, BasicParameter))]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct SharedBoolParameter(Rc<BoolParameter>);

impl Deref for SharedBoolParameter {
    type Target = BoolParameter;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<BoolParameter> for SharedBoolParameter {
    fn from(value: BoolParameter) -> Self {
        Self { 0: Rc::new(value) }
    }
}

impl TryFrom<BasicParameter> for SharedBoolParameter {
    type Error = anyhow::Error;

    fn try_from(value: BasicParameter) -> Result<Self, Self::Error> {
        match value {
            BasicParameter::Bool(p) => Ok(p),
            _ => bail!("unable to downcast"),
        }
    }
}