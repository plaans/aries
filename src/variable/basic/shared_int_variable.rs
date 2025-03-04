use std::rc::Rc;

use anyhow::bail;

use crate::variable::BasicVariable;
use crate::variable::IntVariable;

pub type SharedIntVariable = Rc<IntVariable>;

impl TryFrom<BasicVariable> for SharedIntVariable {
    type Error = anyhow::Error;

    fn try_from(value: BasicVariable) -> Result<Self, Self::Error> {
        match value {
            BasicVariable::Int(int_variable) => Ok(int_variable),
            _ => bail!("unable to downcast"),
        }
    }
}