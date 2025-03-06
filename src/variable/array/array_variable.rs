use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::array::SharedArrayBoolVariable;
use crate::variable::array::SharedArrayIntVariable;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ArrayVariable {
    Bool(SharedArrayBoolVariable),
    Int(SharedArrayIntVariable),
}

impl Identifiable for ArrayVariable {
    fn id(&self) -> &Id {
        match self {
            ArrayVariable::Bool(b) => b.id(),
            ArrayVariable::Int(i) => i.id(),
        }
    }
}