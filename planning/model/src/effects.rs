use crate::{env::Env, *};
use itertools::Itertools;

#[derive(Debug, Clone)]
pub struct StateVariable {
    pub fluent: FluentId,
    pub arguments: SeqExprId,
    #[allow(unused)]
    src: Span,
}

impl<'env> Display for Env<'env, &StateVariable> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            (self.env / self.elem.fluent).name(),
            self.elem.arguments.iter().map(|&a| self.env / a).format(", ")
        )
    }
}

impl StateVariable {
    pub fn new(fluent: FluentId, args: SeqExprId, src: Span) -> Self {
        StateVariable {
            fluent,
            arguments: args,
            src,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EffectOp {
    Assign(ExprId),
    Increase(ExprId),
}

impl<'env> Display for Env<'env, &EffectOp> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.elem {
            EffectOp::Assign(expr_id) => write!(f, ":= {}", self.env / *expr_id),
            EffectOp::Increase(delta) => write!(f, "+= {}", self.env / *delta),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub timing: Timestamp,
    pub state_variable: StateVariable,
    pub operation: EffectOp,
}

impl Effect {
    pub fn assignement(timing: Timestamp, state_variable: StateVariable, value: ExprId) -> Self {
        Effect {
            timing,
            state_variable,
            operation: EffectOp::Assign(value),
        }
    }
    pub fn increase(timing: Timestamp, state_variable: StateVariable, delta: ExprId) -> Self {
        Effect {
            timing,
            state_variable,
            operation: EffectOp::Increase(delta),
        }
    }
}

impl<'env> Display for Env<'env, &Effect> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} {}",
            self.elem.timing,
            self.env / &self.elem.state_variable,
            self.env / &self.elem.operation
        )
    }
}
