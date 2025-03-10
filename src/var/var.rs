use std::rc::Rc;

use transitive::Transitive;

use crate::parvar::ParVar;
use crate::traits::Name;
use crate::var::BasicVar;
use crate::var::BoolArrayVariable;
use crate::var::VarBool;
use crate::var::IntArrayVariable;
use crate::var::VarInt;


#[derive(Transitive)]
#[transitive(from(VarBool, Rc<VarBool>))]
#[transitive(from(VarInt, Rc<VarInt>))]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Var {
    Bool(Rc<VarBool>),
    Int(Rc<VarInt>),
    BoolArray(Rc<BoolArrayVariable>),
    IntArray(Rc<IntArrayVariable>),
}

impl Name for Var {
    fn name(&self) -> &Option<String> {
        match self {
            Var::Bool(v) => v.name(),
            Var::Int(v) => v.name(),
            Var::BoolArray(v) => v.name(),
            Var::IntArray(v) => v.name(),
        }
    }
}

impl From<Rc<VarBool>> for Var {
    fn from(value: Rc<VarBool>) -> Self {
        Self::Bool(value)
    }
}

impl From<Rc<VarInt>> for Var {
    fn from(value: Rc<VarInt>) -> Self {
        Self::Int(value)
    }
}

impl From<Rc<BoolArrayVariable>> for Var {
    fn from(value: Rc<BoolArrayVariable>) -> Self {
        Self::BoolArray(value)
    }
}

impl From<Rc<IntArrayVariable>> for Var {
    fn from(value: Rc<IntArrayVariable>) -> Self {
        Self::IntArray(value)
    }
}

impl From<BasicVar> for Var {
    fn from(value: BasicVar) -> Self {
        match value {
            BasicVar::Bool(v) => v.into(),
            BasicVar::Int(v) => v.into(),
        }
    }
}

impl TryFrom<ParVar> for Var {
    type Error = anyhow::Error;

    fn try_from(value: ParVar) -> Result<Self, Self::Error> {
        match value {
            ParVar::Var(v) => Ok(v),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Var> for Rc<VarBool> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::Bool(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Var> for Rc<VarInt> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::Int(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Var> for Rc<BoolArrayVariable> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::BoolArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Var> for Rc<IntArrayVariable> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::IntArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}