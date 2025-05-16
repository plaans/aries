use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::constraint::Min;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::parser::vec_var_int_from_expr;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

/// Integer array minimum constraint.
///
/// ```flatzinc
/// constraint array_int_minimum([x,y,3], z);
/// % z = min(x,y,3)
/// ```
#[derive(Clone, Debug)]
pub struct ArrayIntMinimum {
    m: Rc<VarInt>,
    x: Vec<Rc<VarInt>>,
}

impl ArrayIntMinimum {
    pub const NAME: &str = "array_int_minimum";
    pub const NB_ARGS: usize = 2;

    pub fn new(m: Rc<VarInt>, x: Vec<Rc<VarInt>>) -> Self {
        Self { m, x }
    }

    pub fn m(&self) -> &Rc<VarInt> {
        &self.m
    }

    pub fn x(&self) -> &Vec<Rc<VarInt>> {
        &self.x
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
        let m = var_int_from_expr(&item.exprs[0], model)?;
        let x = vec_var_int_from_expr(&item.exprs[1], model)?;
        Ok(Self::new(m, x))
    }
}

impl Fzn for ArrayIntMinimum {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.m.fzn(), self.x.fzn())
    }
}

impl TryFrom<Constraint> for ArrayIntMinimum {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayIntMinimum(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayIntMinimum> for Constraint {
    fn from(value: ArrayIntMinimum) -> Self {
        Self::ArrayIntMinimum(value)
    }
}

impl Encode for ArrayIntMinimum {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarInt>| IVar::new(*translation.get(v.id()).unwrap());
        let items = self.x.iter().map(translate).collect();
        Box::new(Min::new(items, translate(&self.m)))
    }
}
