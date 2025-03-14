use std::rc::Rc;

use flatzinc::ConstraintItem;

use crate::adapter::var_bool_from_expr;
use crate::constraint::Constraint;
use crate::model::Model;
use crate::traits::Flatzinc;
use crate::traits::Name;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolEq {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolEq {
    pub const NAME: &str = "bool_eq";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn try_from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(&item.exprs[0], model)?;
        let b = var_bool_from_expr(&item.exprs[1], model)?;
        Ok(Self::new(a, b))
    }
}

impl Flatzinc for BoolEq {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.name(), self.b.name())
    }
}

impl TryFrom<Constraint> for BoolEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolEq(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolEq> for Constraint {
    fn from(value: BoolEq) -> Self {
        Self::BoolEq(value)
    }
}
