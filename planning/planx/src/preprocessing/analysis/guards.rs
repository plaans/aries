//! Conditions of an action on state variables, and the effects they *guard*.
//!
//! An effect of an action on a state variable `sv` starting at `t` is *guarded* if the action requires `sv` to have
//! a value in the state just before `t`, i.e. a condition of the action on `sv` holds over an interval containing
//! `t` or an open interval ending at `t` (see [`ActionTiming::holds_before`]).
//! No other action may then act on `sv` at `t`, as it would interfere with this requirement or this effect.

use crate::*;

use super::{ActionTiming, NumericBounds, Term, conjuncts};

/// The value required by a condition on a state variable.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RequiredValue {
    /// Implicit value of a boolean atom: `true` for `sv` and `false` for `(not sv)`.
    Bool(bool),
    /// Explicit value `v` of an equality `(= sv v)` or `(= v sv)`.
    Expr(ExprId),
}

/// If the expression is of the form `sv`, `(not sv)`, `(= sv v)` or `(= v sv)` (where `v` is not a state variable),
/// returns the state variable and the required value.
pub fn required_value(cond: ExprId, env: &Environment) -> Option<(ExprId, RequiredValue)> {
    let is_sv = |e: ExprId| matches!(env.node(e).expr(), Expr::StateVariable(..));
    match env.node(cond).expr() {
        Expr::StateVariable(..) => Some((cond, RequiredValue::Bool(true))),
        Expr::App(Fun::Not, args) if args.len() == 1 && is_sv(args[0]) => Some((args[0], RequiredValue::Bool(false))),
        Expr::App(Fun::Eq, args) if args.len() == 2 => match (is_sv(args[0]), is_sv(args[1])) {
            (true, false) => Some((args[0], RequiredValue::Expr(args[1]))),
            (false, true) => Some((args[1], RequiredValue::Expr(args[0]))),
            _ => None,
        },
        _ => None,
    }
}

/// A state variable whose value an action requires to be defined over an interval,
/// because a condition of the action reads it as a top-level conjunct: `sv` or `(= sv v)`.
#[derive(Clone, Debug)]
pub struct RequiredStateVariable {
    pub interval: TimeInterval,
    pub fluent: FluentId,
    pub args: Vec<Term>,
    /// The value required: `Bool(true)` for a positive atom `sv`, `Expr(v)` for an equality `(= sv v)`.
    pub value: RequiredValue,
}

/// Returns all state variables (with simple arguments) that the action requires to be defined, with the interval
/// over which they are required (see [`RequiredStateVariable`]).
pub fn required_state_variables(action: &Action, env: &Environment) -> Vec<RequiredStateVariable> {
    let mut res = Vec::new();
    let mut push = |interval: TimeInterval, sv: ExprId, value: RequiredValue| {
        if let Expr::StateVariable(fluent, args) = env.node(sv).expr()
            && let Some(args) = Term::all_of(args, env)
        {
            res.push(RequiredStateVariable {
                interval,
                fluent: *fluent,
                args,
                value,
            });
        }
    };
    for c in &action.conditions {
        for e in conjuncts(c.cond, env) {
            match env.node(e).expr() {
                Expr::StateVariable(..) => push(c.interval, e, RequiredValue::Bool(true)),
                Expr::App(Fun::Eq, args) if args.len() == 2 => {
                    push(c.interval, args[0], RequiredValue::Expr(args[1]));
                    push(c.interval, args[1], RequiredValue::Expr(args[0]));
                }
                _ => {}
            }
        }
    }
    res
}

/// The requirements of an action on state variables, with the means to compare its timepoints.
pub struct ActionGuards {
    pub timing: ActionTiming,
    pub requirements: Vec<RequiredStateVariable>,
}

impl ActionGuards {
    pub fn new(action: &Action, env: &Environment, bounds: &NumericBounds) -> Self {
        ActionGuards {
            timing: ActionTiming::new(action, bounds),
            requirements: required_state_variables(action, env),
        }
    }

    /// The requirements of the action ensuring that `fluent(args)` is defined just before `t`
    /// (i.e. guarding an effect on `fluent(args)` starting at `t`).
    pub fn guards<'a, 'b>(
        &'a self,
        fluent: FluentId,
        args: &'b [Term],
        t: Timestamp,
    ) -> impl Iterator<Item = &'a RequiredStateVariable> + use<'a, 'b> {
        (self.requirements.iter())
            .filter(move |r| r.fluent == fluent && r.args == args && self.timing.holds_before(r.interval, t))
    }

    /// True if an effect on `fluent(args)` starting at `t` is guarded.
    pub fn is_guarded(&self, fluent: FluentId, args: &[Term], t: Timestamp) -> bool {
        self.guards(fluent, args, t).next().is_some()
    }
}
