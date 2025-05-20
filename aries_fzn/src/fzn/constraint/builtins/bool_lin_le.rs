use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::linear::NFLinearSumItem;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::LinLe;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::int_from_expr;
use crate::fzn::parser::vec_int_from_expr;
use crate::fzn::parser::vec_var_bool_from_expr;
use crate::fzn::types::Int;
use crate::fzn::var::VarBool;

/// Boolean linear less or equal constraint.
///
/// ```flatzinc
/// constraint bool_lin_le([1,-1,3], [x,y,z], 5);
/// % x - y + 3*z <= 5
/// % true is 1, false is 0
/// ```
#[derive(Clone, Debug)]
pub struct BoolLinLe {
    a: Vec<Int>,
    b: Vec<Rc<VarBool>>,
    c: Int,
}

impl BoolLinLe {
    pub const NAME: &str = "bool_lin_le";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Int>, b: Vec<Rc<VarBool>>, c: Int) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Int> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarBool>> {
        &self.b
    }

    pub fn c(&self) -> &Int {
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
        let c = int_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(a, b, c))
    }
}

impl Fzn for BoolLinLe {
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

impl TryFrom<Constraint> for BoolLinLe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLinLe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLinLe> for Constraint {
    fn from(value: BoolLinLe) -> Self {
        Self::BoolLinLe(value)
    }
}

impl Encode for BoolLinLe {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate = |v: Rc<VarBool>| translation.get(v.id()).unwrap();
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
