use crate::*;
use derive_more::derive::Display;
use itertools::Itertools;

#[derive(Debug, Clone)]
pub struct StateVariable {
    fluent: Fluent,
    arguments: Vec<TypedExpr>,
    src: Span,
}

impl Display for StateVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.fluent.name(), self.arguments.iter().format(", "))
    }
}

impl StateVariable {
    pub fn new(fluent: Fluent, args: Vec<TypedExpr>, src: Span) -> Self {
        StateVariable {
            fluent,
            arguments: args,
            src,
        }
    }
}

#[derive(Display, Debug, Clone)]
pub enum EffectOp {
    #[display(":= {_0}")]
    Assign(TypedExpr),
}

#[derive(Display, Debug, Clone)]
#[display("[{}] {} {}", timing, state_variable, operation)]
pub struct Effect {
    pub timing: Timestamp,
    pub state_variable: StateVariable,
    pub operation: EffectOp,
}

impl Effect {
    pub fn assignement(timing: Timestamp, state_variable: StateVariable, value: TypedExpr) -> Self {
        Effect {
            timing,
            state_variable,
            operation: EffectOp::Assign(value),
        }
    }
}
