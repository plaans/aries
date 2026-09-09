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
