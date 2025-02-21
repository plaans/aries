use crate::parameter::BoolParameter;
use crate::parameter::IntParameter;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Parameter {
    IntParameter(IntParameter),
    BoolParameter(BoolParameter),
}

impl Identifiable for Parameter {
    fn id(&self) -> &Id {
        match self {
            Parameter::IntParameter(par) => par.id(),
            Parameter::BoolParameter(par) => par.id(),
        }
    }
}