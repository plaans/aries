use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::BVar;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::ClauseReif;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::parser::vec_var_bool_from_expr;
use crate::fzn::var::VarBool;

/// Reified boolean clause constraint.
///
/// ```flatzinc
/// constraint bool_clause_reif([a,b], [c], r);
/// % r <-> a \/ b \/ not c
/// ```
#[derive(Clone, Debug)]
pub struct BoolClauseReif {
    a: Vec<Rc<VarBool>>,
    b: Vec<Rc<VarBool>>,
    r: Rc<VarBool>,
}

impl BoolClauseReif {
    pub const NAME: &str = "bool_clause_reif";
    pub const NB_ARGS: usize = 3;

    pub fn new(
        a: Vec<Rc<VarBool>>,
        b: Vec<Rc<VarBool>>,
        r: Rc<VarBool>,
    ) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Vec<Rc<VarBool>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarBool>> {
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
        let a = vec_var_bool_from_expr(&item.exprs[0], model)?;
        let b = vec_var_bool_from_expr(&item.exprs[1], model)?;
        let r = var_bool_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, r))
    }
}

impl Fzn for BoolClauseReif {
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

impl TryFrom<Constraint> for BoolClauseReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolClauseReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolClauseReif> for Constraint {
    fn from(value: BoolClauseReif) -> Self {
        Self::BoolClauseReif(value)
    }
}

impl Encode for BoolClauseReif {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarBool>| BVar::new(*translation.get(v.id()).unwrap());
        let a = self.a.iter().map(translate).collect();
        let b = self.b.iter().map(translate).collect();
        let r = translate(&self.r);
        let clause_reif = ClauseReif::new(a, b, r);
        Box::new(clause_reif)
    }
}
