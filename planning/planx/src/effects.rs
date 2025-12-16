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
    Decrease(ExprId),
}

impl<'env> Display for Env<'env, &EffectOp> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.elem {
            EffectOp::Assign(expr_id) => write!(f, ":= {}", self.env / *expr_id),
            EffectOp::Increase(delta) => write!(f, "+= {}", self.env / *delta),
            EffectOp::Decrease(delta) => write!(f, "-= {}", self.env / *delta),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpleEffect {
    pub timing: Timestamp,
    /// If set, corresponds to a conditional effect that is only true if the
    /// conditions holds at `timing`
    pub condition: Option<ExprId>,
    pub state_variable: StateVariable,
    pub operation: EffectOp,
}

impl SimpleEffect {
    pub fn assignement(timing: Timestamp, state_variable: StateVariable, value: ExprId) -> Self {
        SimpleEffect {
            timing,
            condition: None,
            state_variable,
            operation: EffectOp::Assign(value),
        }
    }
    pub fn increase(timing: Timestamp, state_variable: StateVariable, delta: ExprId) -> Self {
        SimpleEffect {
            timing,
            condition: None,
            state_variable,
            operation: EffectOp::Increase(delta),
        }
    }
    pub fn decrease(timing: Timestamp, state_variable: StateVariable, delta: ExprId) -> Self {
        SimpleEffect {
            timing,
            condition: None,
            state_variable,
            operation: EffectOp::Decrease(delta),
        }
    }

    pub fn with_condition(mut self, cond: ExprId) -> Self {
        assert!(self.condition.is_none());
        self.condition = Some(cond);
        self
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
        )?;
        if let Some(cond) = self.elem.condition {
            write!(f, " | when {}", self.env / cond)?;
        }
        Ok(())
    }
}

/// A potentially universally quantified effect
#[derive(Debug, Clone)]
pub struct Effect {
    /// Universally quantified variables in the effect expression
    pub universal_quantification: Vec<Param>,
    pub effect_expression: SimpleEffect,
}

impl Effect {
    pub fn with_quantification(mut self, additional_params: &[Param]) -> Self {
        self.universal_quantification.extend_from_slice(additional_params);
        self
    }

    /// Make the underlying effet conditional
    ///
    /// Note: panics if the effect was already conditional
    pub fn with_condition(self, cond: ExprId) -> Self {
        Self {
            universal_quantification: self.universal_quantification,
            effect_expression: self.effect_expression.with_condition(cond),
        }
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
