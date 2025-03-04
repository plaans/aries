use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::array::ArrayBoolVariable;
use crate::variable::array::ArrayIntVariable;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ArrayVariable {
    Bool(ArrayBoolVariable),
    Int(ArrayIntVariable),
}

impl Identifiable for ArrayVariable {
    fn id(&self) -> &Id {
        match self {
            ArrayVariable::Bool(b) => b.id(),
            ArrayVariable::Int(i) => i.id(),
        }
    }
}