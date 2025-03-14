// ------------------ empty/builtins/mod.rs -------------------
mod array_int_element;
mod array_var_int_element;
mod int_abs;
mod int_div;
mod int_eq;
mod int_eq_reif;
mod int_le;
mod int_le_reif;
mod int_lin_eq;
mod int_lin_eq_reif;
mod int_lin_le;
mod int_lin_le_reif;
mod int_lin_ne;
mod int_lin_ne_reif;
mod int_lt;
mod int_lt_reif;
mod int_max;
mod int_min;
mod int_mod;
mod int_ne;
mod int_ne_reif;
mod int_plus;
mod int_pow;
mod int_times;
mod array_bool_and;
mod array_bool_element;
mod array_bool_xor;
mod array_var_bool_element;
mod bool2int;
mod bool_and;
mod bool_clause;
mod bool_eq;
mod bool_eq_reif;
mod bool_le;
mod bool_le_reif;
mod bool_lin_eq;
mod bool_lin_le;
mod bool_lt;
mod bool_lt_reif;
mod bool_not;
mod bool_or;
mod bool_xor;

pub use array_int_element::ArrayIntElement;
pub use array_var_int_element::ArrayVarIntElement;
pub use int_abs::IntAbs;
pub use int_div::IntDiv;
pub use int_eq::IntEq;
pub use int_eq_reif::IntEqReif;
pub use int_le::IntLe;
pub use int_le_reif::IntLeReif;
pub use int_lin_eq::IntLinEq;
pub use int_lin_eq_reif::IntLinEqReif;
pub use int_lin_le::IntLinLe;
pub use int_lin_le_reif::IntLinLeReif;
pub use int_lin_ne::IntLinNe;
pub use int_lin_ne_reif::IntLinNeReif;
pub use int_lt::IntLt;
pub use int_lt_reif::IntLtReif;
pub use int_max::IntMax;
pub use int_min::IntMin;
pub use int_mod::IntMod;
pub use int_ne::IntNe;
pub use int_ne_reif::IntNeReif;
pub use int_plus::IntPlus;
pub use int_pow::IntPow;
pub use int_times::IntTimes;
pub use array_bool_and::ArrayBoolAnd;
pub use array_bool_element::ArrayBoolElement;
pub use array_bool_xor::ArrayBoolXor;
pub use array_var_bool_element::ArrayVarBoolElement;
pub use bool2int::Bool2int;
pub use bool_and::BoolAnd;
pub use bool_clause::BoolClause;
pub use bool_eq::BoolEq;
pub use bool_eq_reif::BoolEqReif;
pub use bool_le::BoolLe;
pub use bool_le_reif::BoolLeReif;
pub use bool_lin_eq::BoolLinEq;
pub use bool_lin_le::BoolLinLe;
pub use bool_lt::BoolLt;
pub use bool_lt_reif::BoolLtReif;
pub use bool_not::BoolNot;
pub use bool_or::BoolOr;
pub use bool_xor::BoolXor;

// ------------------- empty/constraint.rs --------------------
use crate::constraint::builtins::*;

#[derive(Clone, Debug)]
pub enum Constraint {
    ArrayIntElement(ArrayIntElement),
    ArrayVarIntElement(ArrayVarIntElement),
    IntAbs(IntAbs),
    IntDiv(IntDiv),
    IntEq(IntEq),
    IntEqReif(IntEqReif),
    IntLe(IntLe),
    IntLeReif(IntLeReif),
    IntLinEq(IntLinEq),
    IntLinEqReif(IntLinEqReif),
    IntLinLe(IntLinLe),
    IntLinLeReif(IntLinLeReif),
    IntLinNe(IntLinNe),
    IntLinNeReif(IntLinNeReif),
    IntLt(IntLt),
    IntLtReif(IntLtReif),
    IntMax(IntMax),
    IntMin(IntMin),
    IntMod(IntMod),
    IntNe(IntNe),
    IntNeReif(IntNeReif),
    IntPlus(IntPlus),
    IntPow(IntPow),
    IntTimes(IntTimes),
    ArrayBoolAnd(ArrayBoolAnd),
    ArrayBoolElement(ArrayBoolElement),
    ArrayBoolXor(ArrayBoolXor),
    ArrayVarBoolElement(ArrayVarBoolElement),
    Bool2int(Bool2int),
    BoolAnd(BoolAnd),
    BoolClause(BoolClause),
    BoolEq(BoolEq),
    BoolEqReif(BoolEqReif),
    BoolLe(BoolLe),
    BoolLeReif(BoolLeReif),
    BoolLinEq(BoolLinEq),
    BoolLinLe(BoolLinLe),
    BoolLt(BoolLt),
    BoolLtReif(BoolLtReif),
    BoolNot(BoolNot),
    BoolOr(BoolOr),
    BoolXor(BoolXor),
}

// ----------------------- empty/mod.rs -----------------------
pub mod builtins;
mod constraint;

pub use constraint::Constraint;

// ----------- empty/builtins/array_int_element.rs ------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct ArrayIntElement {
    b: Rc<VarInt>,
    a: Vec<Rc<ParInt>>,
    c: Rc<VarInt>,
}

impl ArrayIntElement {
    pub const NAME: &str = "array_int_element";
    pub const NB_ARGS: usize = 3;

    pub fn new(b: Rc<VarInt>, a: Vec<Rc<ParInt>>, c: Rc<VarInt>) -> Self {
        Self { b, a, c }
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let b = var_int_from_expr(item.exprs[0], model);
        let a = par_int_array_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(b, a, c));
    }
}

impl TryFrom<Constraint> for ArrayIntElement {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayIntElement(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayIntElement> for Constraint {
    fn from(value: ArrayIntElement) -> Self {
        Self::ArrayIntElement(value)
    }
}

// --------- empty/builtins/array_var_int_element.rs ----------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

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

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let b = var_int_from_expr(item.exprs[0], model);
        let a = var_int_array_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(b, a, c));
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

// ---------------- empty/builtins/int_abs.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntAbs {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntAbs {
    pub const NAME: &str = "int_abs";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for IntAbs {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntAbs(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntAbs> for Constraint {
    fn from(value: IntAbs) -> Self {
        Self::IntAbs(value)
    }
}

// ---------------- empty/builtins/int_div.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntDiv {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntDiv {
    pub const NAME: &str = "int_div";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntDiv {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntDiv(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntDiv> for Constraint {
    fn from(value: IntDiv) -> Self {
        Self::IntDiv(value)
    }
}

// ----------------- empty/builtins/int_eq.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntEq {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntEq {
    pub const NAME: &str = "int_eq";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for IntEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntEq(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntEq> for Constraint {
    fn from(value: IntEq) -> Self {
        Self::IntEq(value)
    }
}

// -------------- empty/builtins/int_eq_reif.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntEqReif {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    r: Rc<VarBool>,
}

impl IntEqReif {
    pub const NAME: &str = "int_eq_reif";
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

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for IntEqReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntEqReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntEqReif> for Constraint {
    fn from(value: IntEqReif) -> Self {
        Self::IntEqReif(value)
    }
}

// ----------------- empty/builtins/int_le.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLe {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntLe {
    pub const NAME: &str = "int_le";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for IntLe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLe> for Constraint {
    fn from(value: IntLe) -> Self {
        Self::IntLe(value)
    }
}

// -------------- empty/builtins/int_le_reif.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLeReif {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    r: Rc<VarBool>,
}

impl IntLeReif {
    pub const NAME: &str = "int_le_reif";
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

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for IntLeReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLeReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLeReif> for Constraint {
    fn from(value: IntLeReif) -> Self {
        Self::IntLeReif(value)
    }
}

// --------------- empty/builtins/int_lin_eq.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinEq {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarInt>>,
    c: Rc<ParInt>,
}

impl IntLinEq {
    pub const NAME: &str = "int_lin_eq";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarInt>>, c: Rc<ParInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_int_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntLinEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinEq(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinEq> for Constraint {
    fn from(value: IntLinEq) -> Self {
        Self::IntLinEq(value)
    }
}

// ------------ empty/builtins/int_lin_eq_reif.rs -------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinEqReif {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarInt>>,
    c: Rc<ParInt>,
    r: Rc<VarBool>,
}

impl IntLinEqReif {
    pub const NAME: &str = "int_lin_eq_reif";
    pub const NB_ARGS: usize = 4;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarInt>>, c: Rc<ParInt>, r: Rc<VarBool>) -> Self {
        Self { a, b, c, r }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_int_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        let r = var_bool_from_expr(item.exprs[3], model);
        Ok(Self::new(a, b, c, r));
    }
}

impl TryFrom<Constraint> for IntLinEqReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinEqReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinEqReif> for Constraint {
    fn from(value: IntLinEqReif) -> Self {
        Self::IntLinEqReif(value)
    }
}

// --------------- empty/builtins/int_lin_le.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinLe {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarInt>>,
    c: Rc<ParInt>,
}

impl IntLinLe {
    pub const NAME: &str = "int_lin_le";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarInt>>, c: Rc<ParInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_int_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
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

// ------------ empty/builtins/int_lin_le_reif.rs -------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinLeReif {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarInt>>,
    c: Rc<ParInt>,
    r: Rc<VarBool>,
}

impl IntLinLeReif {
    pub const NAME: &str = "int_lin_le_reif";
    pub const NB_ARGS: usize = 4;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarInt>>, c: Rc<ParInt>, r: Rc<VarBool>) -> Self {
        Self { a, b, c, r }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_int_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        let r = var_bool_from_expr(item.exprs[3], model);
        Ok(Self::new(a, b, c, r));
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

// --------------- empty/builtins/int_lin_ne.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinNe {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarInt>>,
    c: Rc<ParInt>,
}

impl IntLinNe {
    pub const NAME: &str = "int_lin_ne";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarInt>>, c: Rc<ParInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_int_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntLinNe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinNe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinNe> for Constraint {
    fn from(value: IntLinNe) -> Self {
        Self::IntLinNe(value)
    }
}

// ------------ empty/builtins/int_lin_ne_reif.rs -------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLinNeReif {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarInt>>,
    c: Rc<ParInt>,
    r: Rc<VarBool>,
}

impl IntLinNeReif {
    pub const NAME: &str = "int_lin_ne_reif";
    pub const NB_ARGS: usize = 4;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarInt>>, c: Rc<ParInt>, r: Rc<VarBool>) -> Self {
        Self { a, b, c, r }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarInt>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_int_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        let r = var_bool_from_expr(item.exprs[3], model);
        Ok(Self::new(a, b, c, r));
    }
}

impl TryFrom<Constraint> for IntLinNeReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLinNeReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLinNeReif> for Constraint {
    fn from(value: IntLinNeReif) -> Self {
        Self::IntLinNeReif(value)
    }
}

// ----------------- empty/builtins/int_lt.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntLt {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntLt {
    pub const NAME: &str = "int_lt";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for IntLt {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntLt(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntLt> for Constraint {
    fn from(value: IntLt) -> Self {
        Self::IntLt(value)
    }
}

// -------------- empty/builtins/int_lt_reif.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;
use crate::var::VarInt;

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

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
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

// ---------------- empty/builtins/int_max.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntMax {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntMax {
    pub const NAME: &str = "int_max";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntMax {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntMax(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntMax> for Constraint {
    fn from(value: IntMax) -> Self {
        Self::IntMax(value)
    }
}

// ---------------- empty/builtins/int_min.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntMin {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntMin {
    pub const NAME: &str = "int_min";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntMin {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntMin(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntMin> for Constraint {
    fn from(value: IntMin) -> Self {
        Self::IntMin(value)
    }
}

// ---------------- empty/builtins/int_mod.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntMod {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntMod {
    pub const NAME: &str = "int_mod";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntMod {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntMod(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntMod> for Constraint {
    fn from(value: IntMod) -> Self {
        Self::IntMod(value)
    }
}

// ----------------- empty/builtins/int_ne.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntNe {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
}

impl IntNe {
    pub const NAME: &str = "int_ne";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for IntNe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntNe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntNe> for Constraint {
    fn from(value: IntNe) -> Self {
        Self::IntNe(value)
    }
}

// -------------- empty/builtins/int_ne_reif.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntNeReif {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    r: Rc<VarBool>,
}

impl IntNeReif {
    pub const NAME: &str = "int_ne_reif";
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

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for IntNeReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntNeReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntNeReif> for Constraint {
    fn from(value: IntNeReif) -> Self {
        Self::IntNeReif(value)
    }
}

// ---------------- empty/builtins/int_plus.rs ----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntPlus {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntPlus {
    pub const NAME: &str = "int_plus";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntPlus {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntPlus(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntPlus> for Constraint {
    fn from(value: IntPlus) -> Self {
        Self::IntPlus(value)
    }
}

// ---------------- empty/builtins/int_pow.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntPow {
    x: Rc<VarInt>,
    y: Rc<VarInt>,
    z: Rc<VarInt>,
}

impl IntPow {
    pub const NAME: &str = "int_pow";
    pub const NB_ARGS: usize = 3;

    pub fn new(x: Rc<VarInt>, y: Rc<VarInt>, z: Rc<VarInt>) -> Self {
        Self { x, y, z }
    }

    pub fn x(&self) -> &Rc<VarInt> {
        &self.x
    }

    pub fn y(&self) -> &Rc<VarInt> {
        &self.y
    }

    pub fn z(&self) -> &Rc<VarInt> {
        &self.z
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let x = var_int_from_expr(item.exprs[0], model);
        let y = var_int_from_expr(item.exprs[1], model);
        let z = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(x, y, z));
    }
}

impl TryFrom<Constraint> for IntPow {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntPow(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntPow> for Constraint {
    fn from(value: IntPow) -> Self {
        Self::IntPow(value)
    }
}

// --------------- empty/builtins/int_times.rs ----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct IntTimes {
    a: Rc<VarInt>,
    b: Rc<VarInt>,
    c: Rc<VarInt>,
}

impl IntTimes {
    pub const NAME: &str = "int_times";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarInt>, b: Rc<VarInt>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Rc<VarInt> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_int_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
    }
}

impl TryFrom<Constraint> for IntTimes {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::IntTimes(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<IntTimes> for Constraint {
    fn from(value: IntTimes) -> Self {
        Self::IntTimes(value)
    }
}

// ------------- empty/builtins/array_bool_and.rs -------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct ArrayBoolAnd {
    a: Vec<Rc<VarBool>>,
    r: Rc<VarBool>,
}

impl ArrayBoolAnd {
    pub const NAME: &str = "array_bool_and";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Vec<Rc<VarBool>>, r: Rc<VarBool>) -> Self {
        Self { a, r }
    }

    pub fn a(&self) -> &Vec<Rc<VarBool>> {
        &self.a
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_array_from_expr(item.exprs[0], model);
        let r = var_bool_from_expr(item.exprs[1], model);
        Ok(Self::new(a, r));
    }
}

impl TryFrom<Constraint> for ArrayBoolAnd {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayBoolAnd(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayBoolAnd> for Constraint {
    fn from(value: ArrayBoolAnd) -> Self {
        Self::ArrayBoolAnd(value)
    }
}

// ----------- empty/builtins/array_bool_element.rs -----------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParBool;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct ArrayBoolElement {
    b: Rc<VarInt>,
    a: Vec<Rc<ParBool>>,
    c: Rc<VarBool>,
}

impl ArrayBoolElement {
    pub const NAME: &str = "array_bool_element";
    pub const NB_ARGS: usize = 3;

    pub fn new(b: Rc<VarInt>, a: Vec<Rc<ParBool>>, c: Rc<VarBool>) -> Self {
        Self { b, a, c }
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn a(&self) -> &Vec<Rc<ParBool>> {
        &self.a
    }

    pub fn c(&self) -> &Rc<VarBool> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let b = var_int_from_expr(item.exprs[0], model);
        let a = par_bool_array_from_expr(item.exprs[1], model);
        let c = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(b, a, c));
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

// ------------- empty/builtins/array_bool_xor.rs -------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct ArrayBoolXor {
    a: Vec<Rc<VarBool>>,
}

impl ArrayBoolXor {
    pub const NAME: &str = "array_bool_xor";
    pub const NB_ARGS: usize = 1;

    pub fn new(a: Vec<Rc<VarBool>>) -> Self {
        Self { a }
    }

    pub fn a(&self) -> &Vec<Rc<VarBool>> {
        &self.a
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_array_from_expr(item.exprs[0], model);
        Ok(Self::new(a));
    }
}

impl TryFrom<Constraint> for ArrayBoolXor {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayBoolXor(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayBoolXor> for Constraint {
    fn from(value: ArrayBoolXor) -> Self {
        Self::ArrayBoolXor(value)
    }
}

// --------- empty/builtins/array_var_bool_element.rs ---------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct ArrayVarBoolElement {
    b: Rc<VarInt>,
    a: Vec<Rc<VarBool>>,
    c: Rc<VarBool>,
}

impl ArrayVarBoolElement {
    pub const NAME: &str = "array_var_bool_element";
    pub const NB_ARGS: usize = 3;

    pub fn new(b: Rc<VarInt>, a: Vec<Rc<VarBool>>, c: Rc<VarBool>) -> Self {
        Self { b, a, c }
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn a(&self) -> &Vec<Rc<VarBool>> {
        &self.a
    }

    pub fn c(&self) -> &Rc<VarBool> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let b = var_int_from_expr(item.exprs[0], model);
        let a = var_bool_array_from_expr(item.exprs[1], model);
        let c = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(b, a, c));
    }
}

impl TryFrom<Constraint> for ArrayVarBoolElement {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::ArrayVarBoolElement(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<ArrayVarBoolElement> for Constraint {
    fn from(value: ArrayVarBoolElement) -> Self {
        Self::ArrayVarBoolElement(value)
    }
}

// ---------------- empty/builtins/bool2int.rs ----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct Bool2int {
    a: Rc<VarBool>,
    b: Rc<VarInt>,
}

impl Bool2int {
    pub const NAME: &str = "bool2int";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarInt>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarInt> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_int_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for Bool2int {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::Bool2int(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<Bool2int> for Constraint {
    fn from(value: Bool2int) -> Self {
        Self::Bool2int(value)
    }
}

// ---------------- empty/builtins/bool_and.rs ----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolAnd {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
    r: Rc<VarBool>,
}

impl BoolAnd {
    pub const NAME: &str = "bool_and";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>, r: Rc<VarBool>) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for BoolAnd {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolAnd(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolAnd> for Constraint {
    fn from(value: BoolAnd) -> Self {
        Self::BoolAnd(value)
    }
}

// -------------- empty/builtins/bool_clause.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

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

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_array_from_expr(item.exprs[0], model);
        let b = var_bool_array_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
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

// ---------------- empty/builtins/bool_eq.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolEq {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolEq {
    pub const NAME: &str = "bool_eq";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for BoolEq {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolEq(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolEq> for Constraint {
    fn from(value: BoolEq) -> Self {
        Self::BoolEq(value)
    }
}

// -------------- empty/builtins/bool_eq_reif.rs --------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolEqReif {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
    r: Rc<VarBool>,
}

impl BoolEqReif {
    pub const NAME: &str = "bool_eq_reif";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>, r: Rc<VarBool>) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for BoolEqReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolEqReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolEqReif> for Constraint {
    fn from(value: BoolEqReif) -> Self {
        Self::BoolEqReif(value)
    }
}

// ---------------- empty/builtins/bool_le.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolLe {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolLe {
    pub const NAME: &str = "bool_le";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for BoolLe {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLe(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLe> for Constraint {
    fn from(value: BoolLe) -> Self {
        Self::BoolLe(value)
    }
}

// -------------- empty/builtins/bool_le_reif.rs --------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolLeReif {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
    r: Rc<VarBool>,
}

impl BoolLeReif {
    pub const NAME: &str = "bool_le_reif";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>, r: Rc<VarBool>) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for BoolLeReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLeReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLeReif> for Constraint {
    fn from(value: BoolLeReif) -> Self {
        Self::BoolLeReif(value)
    }
}

// -------------- empty/builtins/bool_lin_eq.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarBool;
use crate::var::VarInt;

#[derive(Clone, Debug)]
pub struct BoolLinEq {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarBool>>,
    c: Rc<VarInt>,
}

impl BoolLinEq {
    pub const NAME: &str = "bool_lin_eq";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarBool>>, c: Rc<VarInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarBool>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<VarInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_bool_array_from_expr(item.exprs[1], model);
        let c = var_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
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

// -------------- empty/builtins/bool_lin_le.rs ---------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::par::ParInt;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolLinLe {
    a: Vec<Rc<ParInt>>,
    b: Vec<Rc<VarBool>>,
    c: Rc<ParInt>,
}

impl BoolLinLe {
    pub const NAME: &str = "bool_lin_le";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Vec<Rc<ParInt>>, b: Vec<Rc<VarBool>>, c: Rc<ParInt>) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Vec<Rc<ParInt>> {
        &self.a
    }

    pub fn b(&self) -> &Vec<Rc<VarBool>> {
        &self.b
    }

    pub fn c(&self) -> &Rc<ParInt> {
        &self.c
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = par_int_array_from_expr(item.exprs[0], model);
        let b = var_bool_array_from_expr(item.exprs[1], model);
        let c = par_int_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, c));
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

// ---------------- empty/builtins/bool_lt.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolLt {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolLt {
    pub const NAME: &str = "bool_lt";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for BoolLt {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLt(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLt> for Constraint {
    fn from(value: BoolLt) -> Self {
        Self::BoolLt(value)
    }
}

// -------------- empty/builtins/bool_lt_reif.rs --------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolLtReif {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
    r: Rc<VarBool>,
}

impl BoolLtReif {
    pub const NAME: &str = "bool_lt_reif";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>, r: Rc<VarBool>) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for BoolLtReif {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolLtReif(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolLtReif> for Constraint {
    fn from(value: BoolLtReif) -> Self {
        Self::BoolLtReif(value)
    }
}

// ---------------- empty/builtins/bool_not.rs ----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolNot {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolNot {
    pub const NAME: &str = "bool_not";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for BoolNot {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolNot(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolNot> for Constraint {
    fn from(value: BoolNot) -> Self {
        Self::BoolNot(value)
    }
}

// ---------------- empty/builtins/bool_or.rs -----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolOr {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
    r: Rc<VarBool>,
}

impl BoolOr {
    pub const NAME: &str = "bool_or";
    pub const NB_ARGS: usize = 3;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>, r: Rc<VarBool>) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn r(&self) -> &Rc<VarBool> {
        &self.r
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        let r = var_bool_from_expr(item.exprs[2], model);
        Ok(Self::new(a, b, r));
    }
}

impl TryFrom<Constraint> for BoolOr {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolOr(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolOr> for Constraint {
    fn from(value: BoolOr) -> Self {
        Self::BoolOr(value)
    }
}

// ---------------- empty/builtins/bool_xor.rs ----------------
use std::rc::Rc;

use crate::constraint::Constraint;
use crate::var::VarBool;

#[derive(Clone, Debug)]
pub struct BoolXor {
    a: Rc<VarBool>,
    b: Rc<VarBool>,
}

impl BoolXor {
    pub const NAME: &str = "bool_xor";
    pub const NB_ARGS: usize = 2;

    pub fn new(a: Rc<VarBool>, b: Rc<VarBool>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Rc<VarBool> {
        &self.a
    }

    pub fn b(&self) -> &Rc<VarBool> {
        &self.b
    }

    pub fn from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {
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
        let a = var_bool_from_expr(item.exprs[0], model);
        let b = var_bool_from_expr(item.exprs[1], model);
        Ok(Self::new(a, b));
    }
}

impl TryFrom<Constraint> for BoolXor {
    type Error = anyhow::Error;

    fn try_from(value: Constraint) -> Result<Self, Self::Error> {
        match value {
            Constraint::BoolXor(c) => Ok(c),
            _ => anyhow::bail!("unable to downcast to {}", Self::NAME),
        }
    }
}

impl From<BoolXor> for Constraint {
    fn from(value: BoolXor) -> Self {
        Self::BoolXor(value)
    }
}

