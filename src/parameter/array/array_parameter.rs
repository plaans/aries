use crate::parameter::array::ArrayBoolParameter;
use crate::parameter::array::ArrayIntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ArrayParameter {
    Bool(ArrayBoolParameter),
    Int(ArrayIntParameter),
}

impl Identifiable for ArrayParameter {
    fn id(&self) -> &Id {
        match self {
            ArrayParameter::Bool(b) => b.id(),
            ArrayParameter::Int(i) => i.id(),
        }
    }
}