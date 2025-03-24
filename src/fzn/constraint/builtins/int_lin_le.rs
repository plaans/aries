use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::linear::NFLinearSumItem;
use flatzinc::ConstraintItem;

use crate::aries::constraint::LinLe;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::int_from_expr;
use crate::fzn::parser::vec_int_from_expr;
use crate::fzn::parser::vec_var_int_from_expr;
use crate::fzn::types::Int;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

#[derive(Clone, Debug)]
pub struct IntLinLe {
    a: Vec<Int>,
    b: Vec<Rc<VarInt>>,
    c: Int,
}

impl IntLinLe {
    pub const NAME: &str = "int_lin_le";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Int>, b: Vec<Rc<VarInt>>, c: Int) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Int> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Int {
        &self.c
    }

    pub fn try_from_item(
        item: ConstraintItem,
        model: &Model,
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
        let b = vec_var_int_from_expr(&item.exprs[1], model)?;
        let c = int_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, c))
    }
}

impl Fzn for IntLinLe {
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

impl TryFrom<Constraint> for IntLinLe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinLe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinLe> for Constraint {
    fn from(value: IntLinLe) -> Self {
        Self::IntLinLe(value)
    }
}

impl Encode for IntLinLe {
    fn encode(&self, translation: &HashMap<usize, VarRef>) -> Box<dyn Post<usize>> {
        let translate = |v: Rc<VarInt>| translation.get(v.id()).unwrap();
        let sum = self
            .a
            .iter()
            .zip(self.b.clone())
            .map(|x| NFLinearSumItem {
                var: *translate(x.1),
                factor: *x.0,
            })
            .collect();
        Box::new(LinLe::new(sum, self.c))
    }
}
