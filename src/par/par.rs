use std::rc::Rc;

use transitive::Transitive;

use crate::par::ParBool;
use crate::par::ParBoolArray;
use crate::par::ParInt;
use crate::par::ParIntArray;
use crate::parvar::ParVar;
use crate::traits::Flatzinc;

#[derive(Transitive)]
#[transitive(from(ParBool, Rc<ParBool>))]
#[transitive(from(ParInt, Rc<ParInt>))]
#[transitive(from(ParBoolArray, Rc<ParBoolArray>))]
#[transitive(from(ParIntArray, Rc<ParIntArray>))]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Par {
    Bool(Rc<ParBool>),
    Int(Rc<ParInt>),
    BoolArray(Rc<ParBoolArray>),
    IntArray(Rc<ParIntArray>),
}

impl Par {
    pub fn name(&self) -> &String {
        match self {
            Par::Bool(p) => p.name(),
            Par::Int(p) => p.name(),
            Par::BoolArray(p) => p.name(),
            Par::IntArray(p) => p.name(),
        }
    }
}

impl Flatzinc for Par {
    fn fzn(&self) -> String {
        match self {
            Par::Bool(p) => p.fzn(),
            Par::Int(p) => p.fzn(),
            Par::BoolArray(p) => p.fzn(),
            Par::IntArray(p) => p.fzn(),
        }
    }
}

impl From<Rc<ParBool>> for Par {
    fn from(value: Rc<ParBool>) -> Self {
        Self::Bool(value)
    }
}

impl From<Rc<ParInt>> for Par {
    fn from(value: Rc<ParInt>) -> Self {
        Self::Int(value)
    }
}

impl From<Rc<ParBoolArray>> for Par {
    fn from(value: Rc<ParBoolArray>) -> Self {
        Self::BoolArray(value)
    }
}

impl From<Rc<ParIntArray>> for Par {
    fn from(value: Rc<ParIntArray>) -> Self {
        Self::IntArray(value)
    }
}

impl TryFrom<ParVar> for Par {
    type Error = anyhow::Error;

    fn try_from(value: ParVar) -> Result<Self, Self::Error> {
        match value {
            ParVar::Par(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Par> for Rc<ParBool> {
    type Error = anyhow::Error;

    fn try_from(value: Par) -> Result<Self, Self::Error> {
        match value {
            Par::Bool(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Par> for Rc<ParInt> {
    type Error = anyhow::Error;

    fn try_from(value: Par) -> Result<Self, Self::Error> {
        match value {
            Par::Int(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Par> for Rc<ParBoolArray> {
    type Error = anyhow::Error;

    fn try_from(value: Par) -> Result<Self, Self::Error> {
        match value {
            Par::BoolArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}

impl TryFrom<Par> for Rc<ParIntArray> {
    type Error = anyhow::Error;

    fn try_from(value: Par) -> Result<Self, Self::Error> {
        match value {
            Par::IntArray(p) => Ok(p),
            _ => anyhow::bail!("unable to downcast"),
        }
    }
}
