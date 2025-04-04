use std::collections::HashMap;
use std::rc::Rc;

use aries::core::Lit;
use aries::core::VarRef;
use aries::model::lang::expr::or;
use aries::model::lang::BVar;
use aries::model::Label;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::vec_var_bool_from_expr;
use crate::fzn::var::VarBool;
use crate::fzn::Fzn;

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

/// This struct is only here as an anonymous function.
/// It allows to post BoolClause by enforcing all its literals.
struct Lits {
    literals: Vec<Lit>,
}

impl<Lbl: Label> Post<Lbl> for Lits {
    fn post(&self, model: &mut aries::model::Model<Lbl>) {
        model.enforce(or(self.literals.clone()), []);
    }
}

impl Encode for BoolClause {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarBool>| BVar::new(*translation.get(v.id()).unwrap());
        let mut literals = Vec::with_capacity(self.a.len() + self.b.len());
        for v in &self.a {
            literals.push(translate(v).true_lit());
        }
        for v in &self.b {
            literals.push(translate(v).false_lit());
        }

        Box::new(Lits { literals })
    }
}
