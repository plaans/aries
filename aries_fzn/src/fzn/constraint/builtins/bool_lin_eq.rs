use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::linear::NFLinearSumItem;
use flatzinc::ConstraintItem;

use crate::aries::constraint::LinEq;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::parser::vec_int_from_expr;
use crate::fzn::parser::vec_var_bool_from_expr;
use crate::fzn::types::Int;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

/// Boolean linear equality constraint.
///
/// ```flatzinc
/// constraint bool_lin_eq([1,-1,3], [x,y,z], 5);
/// % x - y + 3*z = 5
/// % true is 1, false is 0
/// ```
#[derive(Clone, Debug)]
pub struct BoolLinEq {
    a: Vec<Int>,
    b: Vec<Rc<VarBool>>,
    c: Rc<VarInt>,
}

impl BoolLinEq {
    pub const NAME: &str = "bool_lin_eq";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Int>, b: Vec<Rc<VarBool>>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Int> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarBool>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
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
        let a = vec_int_from_expr(&item.exprs[0], model)?;
        let b = vec_var_bool_from_expr(&item.exprs[1], model)?;
        let c = var_int_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, c))
    }
}

impl Fzn for BoolLinEq {
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

impl TryFrom<Constraint> for BoolLinEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLinEq(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLinEq> for Constraint {
    fn from(value: BoolLinEq) -> Self {
        Self::BoolLinEq(value)
    }
}

impl Encode for BoolLinEq {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate = |v: Rc<VarBool>| translation.get(v.id()).unwrap();
        let mut sum: Vec<NFLinearSumItem> = self
            .a
            .iter()
            .zip(self.b.clone())
            .map(|x| NFLinearSumItem {
                var: *translate(x.1),
                factor: *x.0,
            })
            .collect();
        sum.push(NFLinearSumItem {
            var: *translation.get(self.c.id()).unwrap(),
            factor: -1,
        });
        Box::new(LinEq::new(sum, 0))
    }
}
