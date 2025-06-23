use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::BVar;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::Clause;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::vec_var_bool_from_expr;
use crate::fzn::var::VarBool;

/// Boolean clause constraint.
///
/// ```flatzinc
/// constraint bool_clause([a,b], [c]);
/// % a \/ b \/ not c
/// ```
#[derive(Clone, Debug)]
pub struct BoolClause {
    a: Vec<Rc<VarBool>>,
    b: Vec<Rc<VarBool>>,
}

impl BoolClause {
    pub const NAME: &str = "bool_clause";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Vec<Rc<VarBool>>, b: Vec<Rc<VarBool>>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Vec<Rc<VarBool>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarBool>> {
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
        let a = vec_var_bool_from_expr(&item.exprs[0], model)?;
        let b = vec_var_bool_from_expr(&item.exprs[1], model)?;
        Ok(Self::new(a, b))
    }
}

impl Fzn for BoolClause {
    fn fzn(&self) -> String {
        format!("{}({:?}, {:?});\n", Self::NAME, self.a.fzn(), self.b.fzn())
    }
}

impl TryFrom<Constraint> for BoolClause {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolClause(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolClause> for Constraint {
    fn from(value: BoolClause) -> Self {
        Self::BoolClause(value)
    }
}

impl Encode for BoolClause {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarBool>| BVar::new(*translation.get(v.id()).unwrap());
        let a = self.a.iter().map(translate).collect();
        let b = self.b.iter().map(translate).collect();
        let clause = Clause::new(a, b);
        Box::new(clause)
    }
}
