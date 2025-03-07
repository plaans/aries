use std::rc::Rc;

use anyhow::ensure;

use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::variable::IntVariable;

const NAME: &str = "int_eq";

pub struct IntEq {
    a: Rc<IntVariable>,
    b: Rc<IntVariable>,
}

impl IntEq {
    pub fn new(a: Rc<IntVariable>, b: Rc<IntVariable>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<IntVariable> {
        &self.a
    }

    pub fn b(&self) -> &Rc<IntVariable> {
        &self.b
    }
}

impl Constraint for IntEq {
    fn build(args: Vec<ParVar>) -> anyhow::Result<Self> {
        ensure!(args.len() == 2);
        let [a,b] = <[_;2]>::try_from(args).unwrap();
        let a = a.try_into()?;
        let b = b.try_into()?;
        Ok(Self { a, b })
    }

    fn name(&self) -> &'static str {
        &NAME
    }
    
    fn args(&self) -> Vec<ParVar> {
        vec![self.a.clone().into(), self.b.clone().into()]
    }
}