pub type AriesSignedVar = aries_solver::prelude::SignedVar;
pub type AriesVar = aries_solver::prelude::Var;
pub type AriesVarIdx = u32;
pub type AriesLit = aries_solver::prelude::Lit;
pub type AriesModelEvent = aries_solver::core::state::Event;

pub use aries_solver::prelude::{INT_CST_MAX, INT_CST_MIN, IntCst};

pub type LpCol = highs::Col;
pub type LpRow = highs::Row;
pub type LpSolution = highs::Solution;
pub type LpIis = highs::Iis;
pub type LpModel = highs::Model;
pub type LpObjectiveSense = highs::Sense;

pub type FloatCst = f64;

pub fn float_as_exact_int_cst(value: FloatCst) -> IntCst {
    debug_assert!(value.fract().abs() < 1e-6);
    let v = (value.clamp(INT_CST_MIN as FloatCst, INT_CST_MAX as FloatCst) as IntCst).clamp(INT_CST_MIN, INT_CST_MAX);
    debug_assert!((value - v as FloatCst).abs() < 1e-6);
    v
}
pub fn float_as_floor_int_cst(value: FloatCst) -> IntCst {
    float_as_exact_int_cst(value.floor())
}
pub fn float_as_ceil_int_cst(value: FloatCst) -> IntCst {
    float_as_exact_int_cst(value.ceil())
}

pub fn int_cst_as_float(value: IntCst) -> FloatCst {
    let v = value as FloatCst;
    debug_assert!(float_as_exact_int_cst(v) == value);
    v
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LpLit {
    pub col: LpCol,
    pub tpe: LpLitType,
    pub val: IntCst,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LpLitType {
    GEQ,
    LEQ,
}
impl LpLitType {
    pub fn side(&self) -> usize {
        match self {
            LpLitType::GEQ => 0,
            LpLitType::LEQ => 1,
        }
    }
}

impl LpLit {
    pub fn new(col: LpCol, tpe: LpLitType, val: IntCst) -> Self {
        Self { col, tpe, val }
    }
    pub fn from_model_lit(col: LpCol, lit: AriesLit) -> Self {
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
    pub fn into_model_lit(self, var: AriesVar) -> AriesLit {
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

#[derive(Debug, Clone)]
pub(super) struct LpEvent {
    pub new_lp_lit: LpLit,
    pub cause: LpEventCause,
    pub prev_lp_lit: LpLit,
    pub prev_event: Option<aries_solver::backtrack::EventIndex>,
}

#[derive(Debug, Clone)]
pub(super) enum LpEventCause {
    Binding { scope: AriesLit, main: AriesLit },
    // ReducedCostStrengthtening(Vec<LpLit>),
}

pub(super) enum LpBoundUpdate {
    Unchanged,
    Tightened,
    /// Inconsistent update (lower bound higher than upper bound). Cause is handed back to explain the conflict.
    Emptied(LpEventCause),
}
