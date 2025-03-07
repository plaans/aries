use std::rc::Rc;

use anyhow::bail;
use anyhow::ensure;

use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolAnd {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolAnd {
    pub const NAME: &str = "bool_and";

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    fn build(args: Vec<ParVar>) -> anyhow::Result<Self> {
        ensure!(args.len() == 2);
        let [a,b] = <[_;2]>::try_from(args).unwrap();
        let a = a.try_into()?;
        let b = b.try_into()?;
        Ok(Self { a, b })
    }
}

impl TryFrom<Constraint> for BoolAnd {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolAnd(c) => Ok(c),
            _ => bail!("unable to downcast"),
        }
    }
}