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
    /// States that the state variable's value is undefined (until the next (non-erase) effect)
    Erase,
}

impl<'env> Display for Env<'env, &EffectOp> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.elem {
            EffectOp::Assign(expr_id) => write!(f, ":= {}", self.env / *expr_id),
            EffectOp::Increase(delta) => write!(f, "+= {}", self.env / *delta),
            EffectOp::Decrease(delta) => write!(f, "-= {}", self.env / *delta),
            EffectOp::Erase => write!(f, ":= ∅"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpleEffect {
    /// Time at which the effect start its transition for the previous value.
    /// At `transition_start`, the state variable still has its previous value and will lose it immediately afterwards.
    pub transition_start: Timestamp,
    /// Time at which the effect is established.
    pub transition_end: Timestamp,
    /// If set, corresponds to a conditional effect that is only true if the
    /// conditions holds at `timing`
    pub condition: Option<ExprId>,
    pub state_variable: StateVariable,
    pub operation: EffectOp,
}

impl SimpleEffect {
    pub fn assignement(timing: Timestamp, state_variable: StateVariable, value: ExprId) -> Self {
        SimpleEffect {
            transition_start: timing,
            transition_end: timing + Dt,
            condition: None,
            state_variable,
            operation: EffectOp::Assign(value),
        }
    }
    pub fn increase(timing: Timestamp, state_variable: StateVariable, delta: ExprId) -> Self {
        SimpleEffect {
            transition_start: timing,
            transition_end: timing + Dt,
            condition: None,
            state_variable,
            operation: EffectOp::Increase(delta),
        }
    }
    pub fn decrease(timing: Timestamp, state_variable: StateVariable, delta: ExprId) -> Self {
        SimpleEffect {
            transition_start: timing,
            transition_end: timing + Dt,
            condition: None,
            state_variable,
            operation: EffectOp::Decrease(delta),
        }
    }

    /// Turns the effect into an initial effect, whose value is established at the origin.
    ///
    /// Transitions are never instantaneous, so its transition is on `[ORIGIN - dt, ORIGIN]`.
    pub fn as_initial(mut self) -> Self {
        self.transition_start = Timestamp::ORIGIN - Dt;
        self.transition_end = Timestamp::ORIGIN;
        self
    }

    /// Returns true if the effect is part of the initial state, i.e., its value is established at the origin
    /// (see [`SimpleEffect::as_initial`]).
    pub fn is_initial(&self) -> bool {
        self.transition_end == Timestamp::ORIGIN
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
            span: None,
            id: EffectId::fresh(),
            annotations: Default::default(),
        }
    }

    pub fn not_quantified(self) -> Effect {
        Effect {
            universal_quantification: Vec::new(),
            effect_expression: self,
            span: None,
            id: EffectId::fresh(),
            annotations: Default::default(),
        }
    }
}

impl<'env> Display for Env<'env, &SimpleEffect> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.elem.transition_start == self.elem.transition_end {
            write!(f, "[{}] ", self.elem.transition_start)?;
        } else if self.elem.transition_start + Dt == self.elem.transition_end {
            write!(f, "[{}..] ", self.elem.transition_start)?;
        } else {
            write!(f, "[{},{}] ", self.elem.transition_start, self.elem.transition_end)?;
        }
        write!(
            f,
            "{} {}",
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
    pub span: Option<Span>,
    /// Identifier of the effect, unique within its action (preserved by cloning).
    pub id: EffectId,
    /// Additional information on the effect, typically set by preprocessing steps.
    pub annotations: EffectAnnotations,
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
            span: self.span,
            id: self.id,
            annotations: self.annotations,
        }
    }

    /// Set the span of the effect to be the same as `s`
    pub fn with_span(mut self, s: impl Spanned) -> Self {
        self.span = s.span().cloned();
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

impl<'a> Spanned for Env<'a, &Effect> {
    fn span(&self) -> Option<&Span> {
        self.elem.span.as_ref()
    }
}
