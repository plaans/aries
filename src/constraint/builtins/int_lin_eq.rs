use std::rc::Rc;

use flatzinc::ConstraintItem;

use crate::adapter::int_from_expr;
use crate::adapter::vec_int_from_expr;
use crate::adapter::vec_var_int_from_expr;
use crate::constraint::Constraint;
use crate::model::Model;
use crate::traits::Flatzinc;
use crate::types::Int;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinEq {
    a: Vec<Int>,
    b: Vec<Rc<VarInt>>,
    c: Int,
}

impl IntLinEq {
    pub const NAME: &str = "int_lin_eq";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Int>, b: Vec<Rc<VarInt>>, c: Int) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Int> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Int {
        &self.c
    }

    pub fn try_from_item(
        item: ConstraintItem,
        model: &Model,
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
        let a = vec_int_from_expr(&item.exprs[0], model)?;
        let b = vec_var_int_from_expr(&item.exprs[1], model)?;
        let c = int_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, c))
    }
}

impl Flatzinc for IntLinEq {
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

impl TryFrom<Constraint> for IntLinEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinEq(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinEq> for Constraint {
    fn from(value: IntLinEq) -> Self {
        Self::IntLinEq(value)
    }
}
