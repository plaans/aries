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
use crate::fzn::parser::var_bool_from_expr;
use crate::fzn::parser::var_int_from_expr;
use crate::fzn::parser::vec_bool_from_expr;
use crate::fzn::types::as_int;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;
use crate::fzn::Fzn;

/// Element of boolean array constraint.
///
/// ```flatzinc
/// constraint array_bool_element(i, [true, false, false], x);
/// % x = a[i] with a = [true, false, false]
/// ```
///
/// Remark: first index is 1.
#[derive(Clone, Debug)]
pub struct ArrayBoolElement {
    b: Rc<VarInt>,
    a: Vec<bool>,
    c: Rc<VarBool>,
}

impl ArrayBoolElement {
    pub const NAME: &str = "array_bool_element";
    pub const NB_ARGS: usize = 3;

    pub fn new(b: Rc<VarInt>, a: Vec<bool>, c: Rc<VarBool>) -> Self {
        Self { b, a, c }
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn a(&self) -> &Vec<bool> {
        &self.a
    }

    pub fn c(&self) -> &Rc<VarBool> {
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
        let a = vec_bool_from_expr(&item.exprs[1], model)?;
        let c = var_bool_from_expr(&item.exprs[2], model)?;
        Ok(Self::new(b, a, c))
    }
}

impl Fzn for ArrayBoolElement {
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

impl TryFrom<Constraint> for ArrayBoolElement {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayBoolElement(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayBoolElement> for Constraint {
    fn from(value: ArrayBoolElement) -> Self {
        Self::ArrayBoolElement(value)
    }
}

impl Encode for ArrayBoolElement {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        let translate =
            |v: &Rc<VarBool>| IVar::new(*translation.get(v.id()).unwrap());
        let a = self.a.iter().map(|x| as_int(*x).into()).collect();
        let b = translate(&self.c);
        let i = IVar::new(*translation.get(self.b.id()).unwrap()) - 1; // index starts at 1
        Box::new(ArrayElement::new(a, b, i))
    }
}
