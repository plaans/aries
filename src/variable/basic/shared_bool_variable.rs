use std::rc::Rc;

use anyhow::bail;

use crate::variable::BasicVariable;
use crate::variable::BoolVariable;

pub type SharedBoolVariable = Rc<BoolVariable>;

impl TryFrom<BasicVariable> for SharedBoolVariable {
    type Error = anyhow::Error;

    fn try_from(value: BasicVariable) -> Result<Self, Self::Error> {
        match value {
            BasicVariable::Bool(bool_variable) => Ok(bool_variable),
            _ => bail!("unable to downcast"),
        }
    }
}