use std::rc::Rc;

use transitive::Transitive;

use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;

#[derive(Transitive)]
#[transitive(from(BoolVariable, Rc<BoolVariable>))]
#[transitive(from(IntVariable, Rc<IntVariable>))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BasicVariable {
    Bool(Rc<BoolVariable>),
    Int(Rc<IntVariable>),
}

impl Identifiable for BasicVariable {
    fn id(&self) -> &Id {
        match self {
            BasicVariable::Bool(v) => v.id(),
            BasicVariable::Int(v) => v.id(),
        }
    }
}

impl From<Rc<BoolVariable>> for BasicVariable {
    fn from(value: Rc<BoolVariable>) -> Self {
        Self::Bool(value)
    }
}

impl From<Rc<IntVariable>> for BasicVariable {
    fn from(value: Rc<IntVariable>) -> Self {
        Self::Int(value)
    }
}