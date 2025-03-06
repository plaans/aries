use std::ops::Deref;
use std::rc::Rc;

use anyhow::bail;
use transitive::Transitive;

use crate::parvar::ParVar;
use crate::variable::BasicVariable;
use crate::variable::BoolVariable;
use crate::variable::Variable;

#[derive(Transitive)]
#[transitive(try_from(Variable, BasicVariable))]
#[transitive(try_from(ParVar, Variable, BasicVariable))]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct SharedBoolVariable(Rc<BoolVariable>);

impl Deref for SharedBoolVariable {
    type Target = BoolVariable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<BoolVariable> for SharedBoolVariable {
    fn from(value: BoolVariable) -> Self {
        Self { 0: Rc::new(value) }
    }
}

impl TryFrom<BasicVariable> for SharedBoolVariable {
    type Error = anyhow::Error;

    fn try_from(value: BasicVariable) -> Result<Self, Self::Error> {
        match value {
            BasicVariable::Bool(bool_variable) => Ok(bool_variable),
            _ => bail!("unable to downcast"),
        }
    }
}