use std::rc::Rc;

use transitive::Transitive;

use crate::fzn::parvar::ParVar;
use crate::fzn::var::BasicVar;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarBoolArray;
use crate::fzn::var::VarInt;
use crate::fzn::var::VarIntArray;
use crate::fzn::Fzn;
use crate::fzn::Name;

/// Flatzinc variable.
///
/// ```flatzinc
/// var bool: b;
/// ```
#[derive(Transitive)]
#[transitive(from(VarBool, Rc<VarBool>))]
#[transitive(from(VarInt, Rc<VarInt>))]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Var {
    Bool(Rc<VarBool>),
    Int(Rc<VarInt>),
    BoolArray(Rc<VarBoolArray>),
    IntArray(Rc<VarIntArray>),
}

impl Name for Var {
    fn name(&self) -> &String {
        match self {
            Var::Bool(v) => v.name(),
            Var::Int(v) => v.name(),
            Var::BoolArray(v) => v.name(),
            Var::IntArray(v) => v.name(),
        }
    }
}

impl Fzn for Var {
    fn fzn(&self) -> String {
        match self {
            Var::Bool(v) => v.fzn(),
            Var::Int(v) => v.fzn(),
            Var::BoolArray(v) => v.fzn(),
            Var::IntArray(v) => v.fzn(),
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

impl From<Rc<VarBoolArray>> for Var {
    fn from(value: Rc<VarBoolArray>) -> Self {
        Self::BoolArray(value)
    }
}

impl From<Rc<VarIntArray>> for Var {
    fn from(value: Rc<VarIntArray>) -> Self {
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
            _ => anyhow::bail!("unable to downcast to Var"),
        }
    }
}

impl TryFrom<Var> for Rc<VarBool> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::Bool(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast {value:?} to VarBool"),
        }
    }
}

impl TryFrom<Var> for Rc<VarInt> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::Int(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast to VarInt"),
        }
    }
}

impl TryFrom<Var> for Rc<VarBoolArray> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::BoolArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast to VarBoolArray"),
        }
    }
}

impl TryFrom<Var> for Rc<VarIntArray> {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::IntArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast to VarIntArray"),
        }
    }
}
