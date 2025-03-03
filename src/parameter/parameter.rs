use crate::parameter::BoolParameter;
use crate::parameter::IntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Parameter {
    Int(IntParameter),
    Bool(BoolParameter),
}

impl Identifiable for Parameter {
    fn id(&self) -> &Id {
        match self {
            Parameter::Int(par) => par.id(),
            Parameter::Bool(par) => par.id(),
        }
    }
}