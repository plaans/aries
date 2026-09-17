use aries_solver::core::IntCst;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionGrounding {
    pub args: Vec<IntCst>,
    pub valfrom: Option<IntCst>,
    pub valto: Option<IntCst>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionGroundingId {
    pub state_var_grounding_id: usize,
    pub valfrom: Option<IntCst>,
    pub valto: Option<IntCst>,
}
impl TransitionGroundingId {
    #[allow(dead_code)]
    pub fn is_pure_cond(&self) -> bool {
        self.valfrom.is_some() && self.valto.is_none()
    }
    pub fn is_pure_eff(&self) -> bool {
        self.valfrom.is_none() && self.valto.is_some()
    }
    #[allow(dead_code)]
    pub fn is_condeff(&self) -> bool {
        self.valfrom.is_some() && self.valto.is_some()
    }
}
