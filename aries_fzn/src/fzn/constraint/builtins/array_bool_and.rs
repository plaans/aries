use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::BVar;
use flatzinc::ConstraintItem;

use crate::aries::constraint::AndReif;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::parser::vec_var_bool_from_expr;
use crate::fzn::var::VarBool;
use crate::fzn::Fzn;

/// Boolean arrray and constraint.
///
/// ```flatzinc
/// constraint array_bool_and([a,b,c], r);
/// % r = a /\ b /\ c
/// ```
#[derive(Clone, Debug)]
pub struct ArrayBoolAnd {
    a: Vec<Rc<VarBool>>,
    r: Rc<VarBool>,
}

impl ArrayBoolAnd {
    pub const NAME: &str = "array_bool_and";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Vec<Rc<VarBool>>, r: Rc<VarBool>) -> Self {
        Self { a, r }
    }

    pub fn a(&self) -> &Vec<Rc<VarBool>> {
        &self.a
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
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
        let a = vec_var_bool_from_expr(&item.exprs[0], model)?;
        let r = var_bool_from_expr(&item.exprs[1], model)?;
        Ok(Self::new(a, r))
    }
}

impl Fzn for ArrayBoolAnd {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.r.fzn())
    }
}

impl TryFrom<Constraint> for ArrayBoolAnd {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayBoolAnd(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayBoolAnd> for Constraint {
    fn from(value: ArrayBoolAnd) -> Self {
        Self::ArrayBoolAnd(value)
    }
}

impl Encode for ArrayBoolAnd {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarBool>| BVar::new(*translation.get(v.id()).unwrap());
        let items = self.a.iter().map(translate).collect();
        Box::new(AndReif::new(items, translate(&self.r)))
    }
}
