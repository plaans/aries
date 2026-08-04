use aries_solver::prelude::{INT_CST_MAX, INT_CST_MIN, IntCst};

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
