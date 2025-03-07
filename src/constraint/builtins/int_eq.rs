use std::rc::Rc;

use anyhow::bail;
use anyhow::ensure;

use crate::constraint::Constraint;
use crate::parvar::ParVar;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntEq {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntEq {
    pub const NAME: &str = "int_eq";

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
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

impl TryFrom<Constraint> for IntEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntEq(c) => Ok(c),
            _ => bail!("unable to downcast"),
        }
    }
}