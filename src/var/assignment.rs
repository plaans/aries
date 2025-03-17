use std::rc::Rc;

use crate::types::Int;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Assignment {
    Bool(Rc<VarBool>, bool),
    Int(Rc<VarInt>, Int),
}
