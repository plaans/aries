use anyhow::ensure;

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
    fn create(name: &'static str, args: Vec<ParVar>) -> anyhow::Result<BoolAnd> where Self: Sized {
        ensure!(name == NAME);
        ensure!(args.len() == 2);
        let a = args[0].clone().try_into()?;
        let b = args[1].clone().try_into()?;
        let bool_and = BoolAnd::new(a, b);
        Ok(bool_and)
    }

    fn name(&self) -> &'static str {
        &NAME
    }
    
    fn args(&self) -> impl Iterator<Item = ParVar> {
        [self.a.clone().into(), self.b.clone().into()].into_iter()
    }
}