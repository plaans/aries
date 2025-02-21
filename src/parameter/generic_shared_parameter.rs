use std::rc::Rc;

use crate::parameter::generic_parameter::GenericParameter;
use crate::types::Int;

type GenericSharedParameter<T> = Rc<GenericParameter<T>>;

pub type SharedIntParameter = GenericSharedParameter<Int>;
pub type SharedBoolParameter = GenericSharedParameter<bool>;