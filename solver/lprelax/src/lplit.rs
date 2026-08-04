use aries_solver::prelude::{IntCst, Lit, Var};

use crate::LpCol;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LpLitType {
    GEQ,
    LEQ,
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
            aries_solver::core::Relation::Gt => (LpLitType::GEQ, -lit.ub_value()),
            aries_solver::core::Relation::Leq => (LpLitType::LEQ, lit.ub_value()),
        };
        Self { col, tpe, val }
    }
    pub fn leq(col: LpCol, val: IntCst) -> Self {
        Self {
            col,
            tpe: LpLitType::LEQ,
            val,
        }
    }
    pub fn geq(col: LpCol, val: IntCst) -> Self {
        Self {
            col,
            tpe: LpLitType::GEQ,
            val,
        }
    }
    pub fn into_model_lit(self, var: Var) -> Lit {
        match self.tpe {
            LpLitType::LEQ => var.leq(self.val),
            LpLitType::GEQ => var.geq(self.val),
        }
    }
    pub fn entails(&self, other: Self) -> bool {
        if self.tpe == other.tpe {
            match self.tpe {
                LpLitType::GEQ => self.val >= other.val,
                LpLitType::LEQ => self.val <= other.val,
            }
        } else {
            false
        }
    }
    pub fn strictly_entails(&self, other: Self) -> bool {
        self.entails(other) && self.val != other.val
    }
}
