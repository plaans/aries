use crate::core::{INT_CST_MAX, INT_CST_MIN};
use crate::prelude::{IntCst, Lit, VarRef};

pub use highs::Col as LpCol;
//pub(crate) use highs::Problem as LpProblem;
pub(crate) use highs::Iis as LpIis;
pub(crate) use highs::Model as LpModel;
pub(crate) use highs::RowProblem as LpProblem;
pub use highs::Sense as LpOptimSense;

use smallvec::{SmallVec, smallvec};

pub type FloatCst = f64;
pub fn float_as_exact_int_cst(value: FloatCst) -> IntCst {
    if value <= INT_CST_MIN.into() {
        INT_CST_MIN
    } else if value >= INT_CST_MAX.into() {
        INT_CST_MAX
    } else {
        assert!(value.fract().abs() < 1e-6);
        value as IntCst
    }
}
pub fn float_as_floor_int_cst(value: FloatCst) -> IntCst {
    if value <= INT_CST_MIN.into() {
        INT_CST_MIN
    } else if value >= INT_CST_MAX.into() {
        INT_CST_MAX
    } else {
        value.floor() as IntCst
    }
}
pub fn float_as_ceil_int_cst(value: FloatCst) -> IntCst {
    if value <= INT_CST_MIN.into() {
        INT_CST_MIN
    } else if value >= INT_CST_MAX.into() {
        INT_CST_MAX
    } else {
        value.ceil() as IntCst
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LpLitType {
    LB,
    UB,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LpLit {
    pub col: LpCol,
    pub tpe: LpLitType,
    pub val: IntCst,
}
impl LpLit {
    pub fn new(col: LpCol, tpe: LpLitType, val: IntCst) -> Self {
        Self { col, tpe, val }
    }
    pub fn from_model_lit(col: LpCol, lit: Lit) -> Self {
        let (tpe, val) = match lit.relation() {
            crate::core::Relation::Gt => (LpLitType::LB, -lit.ub_value()),
            crate::core::Relation::Leq => (LpLitType::UB, lit.ub_value()),
        };
        Self { col, tpe, val }
    }
    pub fn leq(col: LpCol, val: IntCst) -> Self {
        Self {
            col,
            tpe: LpLitType::UB,
            val,
        }
    }
    pub fn geq(col: LpCol, val: IntCst) -> Self {
        Self {
            col,
            tpe: LpLitType::LB,
            val,
        }
    }
    pub fn into_model_lit(self, var: VarRef) -> Lit {
        match self.tpe {
            LpLitType::UB => var.leq(self.val),
            LpLitType::LB => var.geq(self.val),
        }
    }
    pub fn entails(&self, other: Self) -> bool {
        if self.tpe == other.tpe {
            match self.tpe {
                LpLitType::LB => self.val >= other.val,
                LpLitType::UB => self.val <= other.val,
            }
        } else {
            false
        }
    }
    pub fn strictly_entails(&self, other: Self) -> bool {
        self.entails(other) && self.val != other.val
    }
}

pub type LitImplierFn = std::sync::Arc<dyn Fn(Lit) -> Option<SmallVec<[LpLit; 4]>> + Send + Sync>;
pub type LpLitImplierFn = std::sync::Arc<dyn Fn(LpLit) -> Option<SmallVec<[Lit; 4]>> + Send + Sync>;

pub fn new_default_lit_implier(var: VarRef, col: LpCol) -> LitImplierFn {
    std::sync::Arc::new(move |lit: Lit| {
        assert_eq!(lit.variable(), var);
        Some(smallvec![LpLit::from_model_lit(col, lit)])
    })
}
pub fn new_default_lplit_implier(var: VarRef, col: LpCol) -> LpLitImplierFn {
    std::sync::Arc::new(move |lplit: LpLit| {
        assert_eq!(lplit.col, col);
        Some(smallvec![lplit.into_model_lit(var)])
    })
}
