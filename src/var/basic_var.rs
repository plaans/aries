use std::rc::Rc;

use transitive::Transitive;

use crate::traits::Name;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Transitive)]
#[transitive(from(VarBool, Rc<VarBool>))]
#[transitive(from(VarInt, Rc<VarInt>))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BasicVar {
    Bool(Rc<VarBool>),
    Int(Rc<VarInt>),
}

impl Name for BasicVar {
    fn name(&self) -> &Option<String> {
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
