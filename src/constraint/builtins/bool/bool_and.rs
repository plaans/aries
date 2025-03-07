use std::rc::Rc;

use anyhow::ensure;

use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::variable::BoolVariable;

const NAME: &str = "bool_and";

pub struct BoolAnd {
    a: Rc<BoolVariable>,
    b: Rc<BoolVariable>,
}

impl BoolAnd {
    pub fn new(a: Rc<BoolVariable>, b: Rc<BoolVariable>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<BoolVariable> {
        &self.a
    }

    pub fn b(&self) -> &Rc<BoolVariable> {
        &self.b
    }
}

impl Constraint for BoolAnd {
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