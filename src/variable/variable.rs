use transitive::Transitive;

use crate::parvar::ParVar;
use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::array::ArrayVariable;
use crate::variable::BasicVariable;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;


#[derive(Transitive)]
#[transitive(from(SharedBoolVariable, BasicVariable))]
#[transitive(from(SharedIntVariable, BasicVariable))]
#[transitive(from(BoolVariable, SharedBoolVariable, BasicVariable))]
#[transitive(from(IntVariable, SharedIntVariable, BasicVariable))]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Variable {
    Basic(BasicVariable),
    Array(ArrayVariable),
}

impl Identifiable for Variable {
    fn id(&self) -> &Id {
        match self {
            Variable::Basic(b) => b.id(),
            Variable::Array(a) => a.id(),
        }
    }
}

impl From<BasicVariable> for Variable {
    fn from(value: BasicVariable) -> Self {
        Self::Basic(value)
    }
}

impl From<ArrayVariable> for Variable {
    fn from(value: ArrayVariable) -> Self {
        Self::Array(value)
    }
}

impl TryFrom<ParVar> for Variable {
    type Error = anyhow::Error;

    fn try_from(value: ParVar) -> Result<Self, Self::Error> {
        match value {
            ParVar::Var(v) => Ok(v),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}