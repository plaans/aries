use std::rc::Rc;

use crate::fzn::types::Int;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;
use crate::fzn::var::VarIntArray;
use crate::fzn::Fzn;
use crate::fzn::Name;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Assignment {
    Bool(Rc<VarBool>, bool),
    Int(Rc<VarInt>, Int),
    IntArray(Rc<VarIntArray>, Vec<Int>),
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
