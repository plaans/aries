use std::rc::Rc;

use transitive::Transitive;

use crate::par::Par;
use crate::var::Var;
use crate::var::VarBool;
use crate::var::VarInt;

// Workaround to transitive crate issue
// https://github.com/bobozaur/transitive/issues/11
type RcIntVariable = Rc<VarInt>;
type RcBoolVariable = Rc<VarBool>;

#[derive(Transitive)]
#[transitive(from(Rc<VarBool>, Var))]
#[transitive(from(VarBool, Rc<VarBool>, Var))]
#[transitive(from(Rc<VarInt>, Var))]
#[transitive(from(VarInt, Rc<VarInt>, Var))]
#[transitive(try_into(Var, RcIntVariable))]
#[transitive(try_into(Var, RcBoolVariable))]
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
