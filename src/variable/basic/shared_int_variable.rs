use std::ops::Deref;
use std::rc::Rc;

use anyhow::bail;
use transitive::Transitive;

use crate::parvar::ParVar;
use crate::variable::BasicVariable;
use crate::variable::IntVariable;
use crate::variable::Variable;

#[derive(Transitive)]
#[transitive(try_from(Variable, BasicVariable))]
#[transitive(try_from(ParVar, Variable, BasicVariable))]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct SharedIntVariable(Rc<IntVariable>);

impl Deref for SharedIntVariable {
    type Target = IntVariable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<IntVariable> for SharedIntVariable {
    fn from(value: IntVariable) -> Self {
        Self { 0: Rc::new(value) }
    }
}

impl TryFrom<BasicVariable> for SharedIntVariable {
    type Error = anyhow::Error;

    fn try_from(value: BasicVariable) -> Result<Self, Self::Error> {
        match value {
            BasicVariable::Int(int_variable) => Ok(int_variable),
            _ => bail!("unable to downcast"),
        }
    }
}