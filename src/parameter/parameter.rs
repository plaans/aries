use crate::parameter::BoolParameter;
use crate::parameter::IntParameter;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Parameter {
    IntParameter(IntParameter),
    BoolParameter(BoolParameter),
}