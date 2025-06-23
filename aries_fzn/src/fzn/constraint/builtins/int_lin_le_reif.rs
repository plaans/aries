use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::BVar;
use aries::model::lang::linear::NFLinearSumItem;
use flatzinc::ConstraintItem;

use crate::aries::Post;
use crate::aries::constraint::LinLeReif;
use crate::fzn::Fzn;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::int_from_expr;
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::parser::vec_int_from_expr;
use crate::fzn::parser::vec_var_int_from_expr;
use crate::fzn::types::Int;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

/// Reified integer linear less or equal constraint.
///
/// ```flatzinc
/// constraint int_lin_le_reif([1,-1,3], [x,y,z], 5, r);
/// % r <-> x - y + 3*z <= 5
/// ```
#[derive(Clone, Debug)]
pub struct IntLinLeReif {
    a: Vec<Int>,
    b: Vec<Rc<VarInt>>,
    c: Int,
    r: Rc<VarBool>,
}

impl IntLinLeReif {
    pub const NAME: &str = "int_lin_le_reif";
    pub const NB_ARGS: usize = 4;

    pub fn new(
        a: Vec<Int>,
        b: Vec<Rc<VarInt>>,
        c: Int,
        r: Rc<VarBool>,
    ) -> Self {
        Self { a, b, c, r }
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
        let a = vec_int_from_expr(&item.exprs[0], model)?;
        let b = vec_var_int_from_expr(&item.exprs[1], model)?;
        let c = int_from_expr(&item.exprs[2], model)?;
        let r = var_bool_from_expr(&item.exprs[3], model)?;
        Ok(Self::new(a, b, c, r))
    }
}

impl Fzn for IntLinLeReif {
    fn fzn(&self) -> String {
        format!(
            "{}({:?}, {:?}, {:?}, {:?});\n",
            Self::NAME,
            self.a.fzn(),
            self.b.fzn(),
            self.c.fzn(),
            self.r.fzn()
        )
    }
}

impl TryFrom<Constraint> for IntLinLeReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinLeReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinLeReif> for Constraint {
    fn from(value: IntLinLeReif) -> Self {
        Self::IntLinLeReif(value)
    }
}

impl Encode for IntLinLeReif {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate = |vid: &usize| translation.get(vid).unwrap();
        let sum = self
            .a
            .iter()
            .zip(self.b.clone())
            .map(|x| NFLinearSumItem {
                var: *translate(x.1.id()),
                factor: *x.0,
            })
            .collect();
        let r = BVar::new(*translate(self.r.id()));
        Box::new(LinLeReif::new(sum, self.c, r))
    }
}
