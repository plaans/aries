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
pub struct SimpleEffect {
    pub timing: Timestamp,
    pub state_variable: StateVariable,
    pub operation: EffectOp,
}

impl SimpleEffect {
    pub fn assignement(timing: Timestamp, state_variable: StateVariable, value: ExprId) -> Self {
        SimpleEffect {
            timing,
            state_variable,
            operation: EffectOp::Assign(value),
        }
    }
    pub fn increase(timing: Timestamp, state_variable: StateVariable, delta: ExprId) -> Self {
        SimpleEffect {
            timing,
            state_variable,
            operation: EffectOp::Increase(delta),
        }
    }

    /// Universally qualify this effect expression over the given variables.
    ///
    /// If the set of variables is empty, the result is equivalent.
    /// If the set of variables is non empy, the variables should correspond to the ones already present in the goal expression.
    pub fn forall(self, vars: Vec<Param>) -> Effect {
        Effect {
            universal_quantification: vars,
            effect_expression: self,
        }
    }

    pub fn not_quantified(self) -> Effect {
        Effect {
            universal_quantification: Vec::new(),
            effect_expression: self,
        }
    }
}

impl<'env> Display for Env<'env, &SimpleEffect> {
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

/// A potentially universally quantified effect
#[derive(Debug, Clone)]
pub struct Effect {
    /// Universally quantified variables in the effect expression
    universal_quantification: Vec<Param>,
    effect_expression: SimpleEffect,
}

impl Effect {
    pub fn with_quantification(mut self, additional_params: &[Param]) -> Self {
        self.universal_quantification.extend_from_slice(additional_params);
        self
    }
}

impl<'a> Display for Env<'a, &Effect> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.env / &self.elem.effect_expression)?;
        if !self.elem.universal_quantification.is_empty() {
            write!(
                f,
                " | forall ({}) ",
                self.elem.universal_quantification.iter().join(", ")
            )?;
        }
        Ok(())
    }
}
