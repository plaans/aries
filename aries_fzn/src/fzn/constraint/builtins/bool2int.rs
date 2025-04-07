use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::constraint::Eq;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

/// Boolean to integer constraint.
///
/// ```flatzinc
/// constraint bool2int(b,x);
/// % x in {0,1} and (b <-> x = 1)
/// ```
#[derive(Clone, Debug)]
pub struct Bool2Int {
    a: Rc<VarBool>,
    b: Rc<VarInt>,
}

impl Bool2Int {
    pub const NAME: &str = "bool2int";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
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
        let a = var_bool_from_expr(&item.exprs[0], model)?;
        let b = var_int_from_expr(&item.exprs[1], model)?;
        Ok(Self::new(a, b))
    }
}

impl Fzn for Bool2Int {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.b.fzn())
    }
}

impl TryFrom<Constraint> for Bool2Int {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::Bool2Int(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<Bool2Int> for Constraint {
    fn from(value: Bool2Int) -> Self {
        Self::Bool2Int(value)
    }
}

impl Encode for Bool2Int {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        Box::new(Eq::new(IVar::new(*a), IVar::new(*b)))
    }
}
