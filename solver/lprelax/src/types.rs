use aries::prelude::{INT_CST_MAX, INT_CST_MIN, IntCst};

pub type LpCol = highs::Col;
pub type LpIis = highs::Iis;
pub type LpModel = highs::Model;
pub type LpProblem = highs::RowProblem;
pub type LpObjectiveSense = highs::Sense;

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
