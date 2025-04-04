use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::constraint::Ne;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

/// Integer not equal constraint.
///
/// ```flatzinc
/// constraint int_ne(x,y);
/// % x != y
/// ```
#[derive(Clone, Debug)]
pub struct IntNe {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntNe {
    pub const NAME: &str = "int_ne";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
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
        Ok(Self::new(a, b))
    }
}

impl Fzn for IntNe {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.b.fzn())
    }
}

impl TryFrom<Constraint> for IntNe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntNe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntNe> for Constraint {
    fn from(value: IntNe) -> Self {
        Self::IntNe(value)
    }
}

impl Encode for IntNe {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        Box::new(Ne::new(IVar::new(*a), IVar::new(*b)))
    }
}
