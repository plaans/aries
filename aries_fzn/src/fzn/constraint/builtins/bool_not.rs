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
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::var::VarBool;
use crate::fzn::Fzn;

/// Boolean not constraint.
///
/// ```flatzinc
/// constraint bool_not(x,y);
/// % x != y
/// ```
#[derive(Clone, Debug)]
pub struct BoolNot {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolNot {
    pub const NAME: &str = "bool_not";
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
        let a = var_bool_from_expr(&item.exprs[0], model)?;
        let b = var_bool_from_expr(&item.exprs[1], model)?;
        Ok(Self::new(a, b))
    }
}

impl Fzn for BoolNot {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.b.fzn())
    }
}

impl TryFrom<Constraint> for BoolNot {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolNot(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolNot> for Constraint {
    fn from(value: BoolNot) -> Self {
        Self::BoolNot(value)
    }
}

impl Encode for BoolNot {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        Box::new(Ne::new(IVar::new(*a), IVar::new(*b)))
    }
}
