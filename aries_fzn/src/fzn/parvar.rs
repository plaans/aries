use std::rc::Rc;

use transitive::Transitive;

use crate::fzn::par::Par;
use crate::fzn::var::Var;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

#[derive(Transitive)]
#[transitive(from(Rc<VarBool>, Var))]
#[transitive(from(VarBool, Rc<VarBool>, Var))]
#[transitive(from(Rc<VarInt>, Var))]
#[transitive(from(VarInt, Rc<VarInt>, Var))]
#[transitive(try_into(Var, Rc<VarInt>))]
#[transitive(try_into(Var, Rc<VarBool>))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParVar {
    Par(Par),
    Var(Var),
}

impl From<Par> for ParVar {
    fn from(value: Par) -> Self {
        Self::Par(value)
    }
}

impl From<Var> for ParVar {
    fn from(value: Var) -> Self {
        Self::Var(value)
    }
}
