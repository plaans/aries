use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::Lt;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::var::VarInt;

/// Integer less than constraint.
///
/// ```flatzinc
/// constraint int_lt(x,y);
/// % x < y
/// ```
#[derive(Clone, Debug)]
pub struct IntLt {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntLt {
    pub const NAME: &str = "int_lt";
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

impl Fzn for IntLt {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.b.fzn())
    }
}

impl TryFrom<Constraint> for IntLt {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLt(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLt> for Constraint {
    fn from(value: IntLt) -> Self {
        Self::IntLt(value)
    }
}

impl Encode for IntLt {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        Box::new(Lt::new(IVar::new(*a), IVar::new(*b)))
    }
}
