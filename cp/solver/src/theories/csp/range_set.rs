use aries::model::lang::IntCst;

pub struct RangeSet {
    values: Vec<i32>,
}

impl RangeSet {
    pub fn empty() -> Self {
        RangeSet { values: Vec::new() }
    }

    pub fn universe() -> Self {
        Self::new(IntCst::MIN, IntCst::MAX)
    }

    pub fn new(lb: IntCst, ub: IntCst) -> Self {
        assert!(lb <= ub);
        RangeSet { values: vec![lb, ub] }
    }
}
