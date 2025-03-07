use std::rc::Rc;

use anyhow::ensure;

use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::var::VarBool;

const NAME: &str = "bool_and";

pub struct BoolAnd {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolAnd {
    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
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