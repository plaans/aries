use std::rc::Rc;

use transitive::Transitive;

use crate::parvar::ParVar;
use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::BoolArrayVariable;
use crate::variable::BoolVariable;
use crate::variable::IntArrayVariable;
use crate::variable::IntVariable;


#[derive(Transitive)]
#[transitive(from(BoolVariable, Rc<BoolVariable>))]
#[transitive(from(IntVariable, Rc<IntVariable>))]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Variable {
    Bool(Rc<BoolVariable>),
    Int(Rc<IntVariable>),
    BoolArray(Rc<BoolArrayVariable>),
    IntArray(Rc<IntArrayVariable>),
}

impl Identifiable for Variable {
    fn id(&self) -> &Id {
        match self {
            Variable::Bool(v) => v.id(),
            Variable::Int(v) => v.id(),
            Variable::BoolArray(v) => v.id(),
            Variable::IntArray(v) => v.id(),
        }
    }
}

impl From<Rc<BoolVariable>> for Variable {
    fn from(value: Rc<BoolVariable>) -> Self {
        Self::Bool(value)
    }
}

impl From<Rc<IntVariable>> for Variable {
    fn from(value: Rc<IntVariable>) -> Self {
        Self::Int(value)
    }
}

impl From<Rc<BoolArrayVariable>> for Variable {
    fn from(value: Rc<BoolArrayVariable>) -> Self {
        Self::BoolArray(value)
    }
}

impl From<Rc<IntArrayVariable>> for Variable {
    fn from(value: Rc<IntArrayVariable>) -> Self {
        Self::IntArray(value)
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

impl TryFrom<Variable> for Rc<BoolVariable> {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::Bool(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Variable> for Rc<IntVariable> {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::Int(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Variable> for Rc<BoolArrayVariable> {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::BoolArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Variable> for Rc<IntArrayVariable> {
    type Error = anyhow::Error;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Variable::IntArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}