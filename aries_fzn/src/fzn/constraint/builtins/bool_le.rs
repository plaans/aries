use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::constraint::Le;
use crate::aries::Post;
use crate::fzn::constraint::Encode;
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::constraint::Constraint;
use crate::fzn::model::Model;
use crate::fzn::Fzn;
use crate::fzn::var::VarBool;

/// Boolean less or equal constraint.
///
/// ```flatzinc
/// constraint bool_le(a,b);
/// % a <= b
/// % a -> b
/// ```
#[derive(Clone, Debug)]
pub struct BoolLe {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolLe {
    pub const NAME: &str = "bool_le";
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

    pub fn try_from_item(item: ConstraintItem, model: &mut Model) -> anyhow::Result<Self> {
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

impl Fzn for BoolLe {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.b.fzn())
    }
}

impl TryFrom<Constraint> for BoolLe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLe> for Constraint {
    fn from(value: BoolLe) -> Self {
        Self::BoolLe(value)
    }
}

impl Encode for BoolLe {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        Box::new(Le::new(IVar::new(*a), IVar::new(*b)))
    }
}
