use std::collections::HashMap;
use std::rc::Rc;

use aries::core::VarRef;
use aries::model::lang::IVar;
use flatzinc::ConstraintItem;

use crate::aries::constraint::ArrayElement;
use crate::aries::Post;
use crate::fzn::constraint::Constraint;
use crate::fzn::constraint::Encode;
use crate::fzn::model::Model;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::parser::vec_var_int_from_expr;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

/// Element of integer variable array constraint.
///
/// ```flatzinc
/// constraint array_var_int_element(i, [a,b,c,d], x);
/// % x = v[i] with v = [a,b,c,d]
/// ```
///
/// Remark: first index is 1.
#[derive(Clone, Debug)]
pub struct ArrayVarIntElement {
    b: Rc<VarInt>,
    a: Vec<Rc<VarInt>>,
    c: Rc<VarInt>,
}

impl ArrayVarIntElement {
    pub const NAME: &str = "array_var_int_element";
    pub const NB_ARGS: usize = 3;

    pub fn new(b: Rc<VarInt>, a: Vec<Rc<VarInt>>, c: Rc<VarInt>) -> Self {
        Self { b, a, c }
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn a(&self) -> &Vec<Rc<VarInt>> {
        &self.a
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
        let b = var_int_from_expr(&item.exprs[0], model)?;
        let a = vec_var_int_from_expr(&item.exprs[1], model)?;
        let c = var_int_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(b, a, c))
    }
}

impl Fzn for ArrayVarIntElement {
    fn fzn(&self) -> String {
        format!(
            "{}({:?}, {:?}, {:?});\n",
            Self::NAME,
            self.b.fzn(),
            self.a.fzn(),
            self.c.fzn()
        )
    }
}

impl TryFrom<Constraint> for ArrayVarIntElement {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayVarIntElement(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayVarIntElement> for Constraint {
    fn from(value: ArrayVarIntElement) -> Self {
        Self::ArrayVarIntElement(value)
    }
}

impl Encode for ArrayVarIntElement {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarInt>| IVar::new(*translation.get(v.id()).unwrap());
        let a = self.a.iter().map(|x| translate(x).into()).collect();
        let b = translate(&self.c);
        let i = translate(&self.b) - 1; // index starts at 1
        Box::new(ArrayElement::new(a, b, i))
    }
}
