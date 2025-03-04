use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::variable::SharedBoolVariable;

const NAME: &str = "bool_and";

pub struct BoolAnd {
    a: SharedBoolVariable,
    b: SharedBoolVariable,
}

impl BoolAnd {
    pub fn new(a: SharedBoolVariable, b: SharedBoolVariable) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &SharedBoolVariable {
        &self.a
    }

    pub fn b(&self) -> &SharedBoolVariable {
        &self.b
    }
}

impl Constraint for BoolAnd {
    fn name(&self) -> &'static str {
        &NAME
    }
    
    fn args(&self) -> Vec<ParVar> {
        vec![self.a.clone().into(), self.b.clone().into()]
    }
}