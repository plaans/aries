use std::rc::Rc;

use crate::traits::Flatzinc;
use crate::traits::Name;
use crate::types::Int;
use crate::var::VarBool;
use crate::var::VarInt;
use crate::var::VarIntArray;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Assignment {
    Bool(Rc<VarBool>, bool),
    Int(Rc<VarInt>, Int),
    IntArray(Rc<VarIntArray>, Vec<Int>),
}

impl Flatzinc for Assignment {
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
