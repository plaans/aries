use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::variable::SharedIntVariable;

const NAME: &str = "int_eq";

pub struct IntEq {
    a: SharedIntVariable,
    b: SharedIntVariable,
}

impl IntEq {
    pub fn new(a: SharedIntVariable, b: SharedIntVariable) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &SharedIntVariable {
        &self.a
    }

    pub fn b(&self) -> &SharedIntVariable {
        &self.b
    }
}

impl Constraint for IntEq {
    fn name(&self) -> &'static str {
        &NAME
    }
    
    fn args(&self) -> Vec<ParVar> {
        vec![self.a.clone().into(), self.b.clone().into()]
    }
}