use std::rc::Rc;

use anyhow::bail;
use transitive::Transitive;

use crate::fzn::var::Var;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;
use crate::fzn::Name;

/// Basic flatzinc variable.
///
/// ```flatzinc
/// var 2..6: x;
/// ```
#[derive(Transitive)]
#[transitive(from(VarBool, Rc<VarBool>))]
#[transitive(from(VarInt, Rc<VarInt>))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BasicVar {
    Bool(Rc<VarBool>),
    Int(Rc<VarInt>),
}

impl BasicVar {
    pub fn id(&self) -> &usize {
        match self {
            BasicVar::Bool(v) => v.id(),
            BasicVar::Int(v) => v.id(),
        }
    }
}

impl Name for BasicVar {
    fn name(&self) -> &String {
        match self {
            BasicVar::Bool(v) => v.name(),
            BasicVar::Int(v) => v.name(),
        }
    }
}

impl From<Rc<VarBool>> for BasicVar {
    fn from(value: Rc<VarBool>) -> Self {
        Self::Bool(value)
    }
}

impl From<Rc<VarInt>> for BasicVar {
    fn from(value: Rc<VarInt>) -> Self {
        Self::Int(value)
    }
}

impl TryFrom<Var> for BasicVar {
    type Error = anyhow::Error;

    fn try_from(value: Var) -> Result<Self, Self::Error> {
        match value {
            Var::Bool(v) => Ok(Self::Bool(v)),
            Var::Int(v) => Ok(Self::Int(v)),
            Var::BoolArray(_) => {
                bail!("unable to downcast bool array to basic var")
            }
            Var::IntArray(_) => {
                bail!("unable to downcast int array to basic var")
            }
        }
    }
}
