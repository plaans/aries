use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::BVar;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::LtReif;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

/// Reified less than constraint.
///
/// ```flatzinc
/// constraint int_lt_reif(x,y,r);
/// % r <-> x < y
/// ```
#[derive(Clone, Debug)]
pub struct IntLtReif {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    r: Rc<VarBool>,
}

impl IntLtReif {
    pub const NAME: &str = "int_lt_reif";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, r: Rc<VarBool>) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
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
        let a = var_int_from_expr(&item.exprs[0], model)?;
        let b = var_int_from_expr(&item.exprs[1], model)?;
        let r = var_bool_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, r))
    }
}

impl Fzn for IntLtReif {
    fn fzn(&self) -> String {
        format!(
            "{}({:?}, {:?}, {:?});\n",
            Self::NAME,
            self.a.fzn(),
            self.b.fzn(),
            self.r.fzn()
        )
    }
}

impl TryFrom<Constraint> for IntLtReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLtReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLtReif> for Constraint {
    fn from(value: IntLtReif) -> Self {
        Self::IntLtReif(value)
    }
}

impl Encode for IntLtReif {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<(dyn Post<usize>)> {
        let a = translation.get(self.a.id()).unwrap();
        let b = translation.get(self.b.id()).unwrap();
        let r = translation.get(self.r.id()).unwrap();
        Box::new(LtReif::new(IVar::new(*a), IVar::new(*b), BVar::new(*r)))
    }
}
