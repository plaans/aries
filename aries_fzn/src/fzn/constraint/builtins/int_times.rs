use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::Mul;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::var::VarInt;

/// Integer multiplication constraint.
///
/// ```flatzinc
/// constraint int_times(x,y,z);
/// % x * y = z
/// ```
#[derive(Clone, Debug)]
pub struct IntTimes {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntTimes {
    pub const NAME: &str = "int_times";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn try_from_item(
        item: ConstraintItem,
        model: &mut Model,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            item.id.as_str() == Self::NAME,
            "'{}' expected but received '{}'",
            Self::NAME,
            item.id,
        );
        anyhow::ensure!(
            item.exprs.len() == Self::NB_ARGS,
            "{} args expected but received {}",
            Self::NB_ARGS,
            item.exprs.len(),
        );
        let a = var_int_from_expr(&item.exprs[0], model)?;
        let b = var_int_from_expr(&item.exprs[1], model)?;
        let c = var_int_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, c))
    }
}

impl Fzn for IntTimes {
    fn fzn(&self) -> String {
        format!(
            "{}({:?}, {:?}, {:?});\n",
            Self::NAME,
            self.a.fzn(),
            self.b.fzn(),
            self.c.fzn()
        )
    }
}

impl TryFrom<Constraint> for IntTimes {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntTimes(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntTimes> for Constraint {
    fn from(value: IntTimes) -> Self {
        Self::IntTimes(value)
    }
}

impl Encode for IntTimes {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        let c = translation.get(self.c.id()).unwrap();
        Box::new(Mul::new(IVar::new(*c), IVar::new(*a), IVar::new(*b)))
    }
}
