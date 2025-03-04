use std::rc::Rc;

use crate::types::Int;
use super::generic_basic_parameter::GenericBasicParameter;

type GenericSharedBasicParameter<T> = Rc<GenericBasicParameter<T>>;

pub type SharedIntParameter = GenericSharedBasicParameter<Int>;
pub type SharedBoolParameter = GenericSharedBasicParameter<bool>;