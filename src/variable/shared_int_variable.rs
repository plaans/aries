use std::rc::Rc;

use anyhow::bail;

use crate::variable::IntVariable;
use crate::variable::Variable;

pub type SharedIntVariable = Rc<IntVariable>;

impl TryFrom<Variable> for SharedIntVariable {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::Int(int_variable) => Ok(int_variable),
            _ => bail!("unable to downcast"),
        }
    }
}