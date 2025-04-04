use std::rc::Rc;

use crate::fzn::types::Int;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;
use crate::fzn::var::VarIntArray;
use crate::fzn::Fzn;
use crate::fzn::Name;

/// Variable assignment.
///
/// It is used to define a solution.
/// ```flatzinc
/// x = 4;
/// ```
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Assignment {
    Bool(Rc<VarBool>, bool),
    Int(Rc<VarInt>, Int),
    IntArray(Rc<VarIntArray>, Vec<Int>),
}

impl Assignment {
    pub fn output(&self) -> bool {
        match self {
            Assignment::Bool(v, _) => v.output(),
            Assignment::Int(v, _) => v.output(),
            Assignment::IntArray(v, _) => v.output(),
        }
    }
}

impl TryFrom<Assignment> for (Rc<VarBool>, bool) {
    type Error = anyhow::Error;

    fn try_from(value: Assignment) -> Result<Self, Self::Error> {
        match value {
            Assignment::Bool(v, x) => Ok((v, x)),
            Assignment::Int(v, _) => {
                anyhow::bail!(format!("{} is not a var bool", v.name()))
            }
            Assignment::IntArray(v, _) => {
                anyhow::bail!(format!("{} is not a var bool", v.name()))
            }
        }
    }
}

impl TryFrom<Assignment> for (Rc<VarInt>, Int) {
    type Error = anyhow::Error;

    fn try_from(value: Assignment) -> Result<Self, Self::Error> {
        match value {
            Assignment::Bool(v, _) => {
                anyhow::bail!(format!("{} is not a var int", v.name()))
            }
            Assignment::Int(v, x) => Ok((v, x)),
            Assignment::IntArray(v, _) => {
                anyhow::bail!(format!("{} is not a var int", v.name()))
            }
        }
    }
}

impl Fzn for Assignment {
    fn fzn(&self) -> String {
        match self {
            Assignment::Bool(var, value) => {
                format!("{} = {};", var.name(), value)
            }
            Assignment::Int(var, value) => {
                format!("{} = {};", var.name(), value)
            }
            Assignment::IntArray(var, value) => {
                format!("{} = {};", var.name(), value.fzn())
            }
        }
    }
}
