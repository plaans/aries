use std::default;

use crate::domain::IntDomain;

pub enum Variable {
    IntVariable(IntVariable),
    BoolVariable(BoolVariable),
}

pub struct IntVariable {
    domain: IntDomain,
}

pub struct BoolVariable;