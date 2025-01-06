use crate::*;

#[derive(Debug, Clone)]
pub struct StateVariable {
    fluent: Fluent,
    arguments: Vec<TypedExpr>,
    src: Span,
}

#[derive(Debug, Clone)]
pub enum EffectOp {
    Assign(TypedExpr),
}

#[derive(Debug, Clone)]
pub struct Effect {
    timing: Timestamp,
    state_variable: StateVariable,
    operation: EffectOp,
}


impl Effect {

    pub fn assignement(timing: Timestamp, )
}
