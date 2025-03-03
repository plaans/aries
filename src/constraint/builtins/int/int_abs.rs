use anyhow::ensure;

use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::variable::SharedIntVariable;

const NAME: &str = "int_abs";

pub struct IntAbs {
    a: SharedIntVariable,
    b: SharedIntVariable,
}

impl IntAbs {
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

impl Constraint for IntAbs {
    fn create(name: &'static str, args: Vec<ParVar>) -> anyhow::Result<IntAbs> where Self: Sized {
        ensure!(name == NAME);
        ensure!(args.len() == 2);
        let a = args[0].clone().try_into()?;
        let b = args[1].clone().try_into()?;
        let bool_and = IntAbs::new(a, b);
        Ok(bool_and)
    }

    fn name(&self) -> &'static str {
        &NAME
    }
    
    fn args(&self) -> impl Iterator<Item = ParVar> {
        [self.a.clone().into(), self.b.clone().into()].into_iter()
    }
}