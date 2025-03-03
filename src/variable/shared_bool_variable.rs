use std::rc::Rc;

use anyhow::bail;

use crate::variable::BoolVariable;
use crate::variable::Variable;

pub type SharedBoolVariable = Rc<BoolVariable>;

impl TryFrom<Variable> for SharedBoolVariable {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::Bool(bool_variable) => Ok(bool_variable),
            _ => bail!("unable to downcast"),
        }
    }
}