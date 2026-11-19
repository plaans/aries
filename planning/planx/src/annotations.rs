//! Annotations attached to the conditions and effects of actions.
//!
//! Annotations record information on the role of a condition or effect in its action (e.g. being part of a
//! transition), typically computed by a preprocessing step to be exploited by later ones or by the encoding.
//! Each kind of annotation is a field of [`ConditionAnnotations`] or [`EffectAnnotations`].
//!
//! # Robustness to model transformations
//!
//! Conditions and effects are referred to by their identifier ([`ConditionId`], [`EffectId`]), assigned at creation
//! and unaffected by the removal or reordering of other elements.
//!
//! Annotations relating several elements are exposed through accessors of [`Action`]
//! (e.g. [`Action::transition_of_condition`]) and are trusted: a transformation applied after an annotation pass is
//! responsible for keeping the annotated patterns valid (e.g. a transformation that preserves the pattern, such as
//! renaming a fluent or rescaling time) or for removing the annotations it invalidates.
//!
//! An invalid annotation is a bug: in debug builds, the accessors check, on the current state of the action, that
//! the annotated pattern still holds and panic otherwise. These checks are skipped in release builds.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::preprocessing::analysis::Term;
use crate::*;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Identifier of a [`Condition`], unique within its action.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConditionId(u64);

impl ConditionId {
    /// A new identifier, distinct from all previously created ones.
    pub fn fresh() -> Self {
        ConditionId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Identifier of an [`Effect`], unique within its action.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(u64);

impl EffectId {
    /// A new identifier, distinct from all previously created ones.
    pub fn fresh() -> Self {
        EffectId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Annotations of a condition of an action (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct ConditionAnnotations {
    /// Effect forming a transition with this condition (see [`annotate_transitions`](crate::preprocessing::annotate_transitions)).
    /// Only read through [`Action::transition_of_condition`], which checks that it is still valid.
    pub(crate) transition: Option<EffectId>,
}

/// Annotations of an effect of an action (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct EffectAnnotations {
    /// Condition forming a transition with this effect (see [`annotate_transitions`](crate::preprocessing::annotate_transitions)).
    /// Only read through [`Action::transition_of_effect`], which checks that it is still valid.
    pub(crate) transition: Option<ConditionId>,
}

/// A condition immediately followed by an assignment of the same state variable.
#[derive(Copy, Clone, Debug)]
pub struct Transition<'a> {
    pub condition: &'a Condition,
    pub effect: &'a Effect,
    /// The value required by the condition.
    pub required: RequiredValue,
    /// The value assigned by the effect.
    pub value: ExprId,
    /// True if the effect establishes the value required by the condition.
    pub borrow: bool,
}

impl Action {
    /// The condition with the given identifier.
    ///
    /// Identifiers are unique within an action (checked in debug builds).
    pub fn condition(&self, id: ConditionId) -> Option<&Condition> {
        let mut matching = self.conditions.iter().filter(|c| c.id == id);
        let first = matching.next();
        debug_assert!(matching.next().is_none(), "Duplicated condition id: {id:?}");
        first
    }

    /// The effect with the given identifier.
    ///
    /// Identifiers are unique within an action (checked in debug builds).
    pub fn effect(&self, id: EffectId) -> Option<&Effect> {
        let mut matching = self.effects.iter().filter(|e| e.id == id);
        let first = matching.next();
        debug_assert!(matching.next().is_none(), "Duplicated effect id: {id:?}");
        first
    }

    /// The transition the condition (of this action) is annotated with.
    ///
    /// # Panics
    /// If the annotation is invalid (only detected in debug builds, except for a missing counterpart).
    pub fn transition_of_condition<'a>(&'a self, cond: &'a Condition, env: &Environment) -> Option<Transition<'a>> {
        let eff_id = cond.annotations.transition?;
        let effect = self
            .effect(eff_id)
            .unwrap_or_else(|| panic!("Transition annotation refers to an absent effect: {eff_id:?}"));
        Some(self.annotated_transition(cond, effect, env))
    }

    /// The transition the effect (of this action) is annotated with.
    ///
    /// # Panics
    /// If the annotation is invalid (only detected in debug builds, except for a missing counterpart).
    pub fn transition_of_effect<'a>(&'a self, eff: &'a Effect, env: &Environment) -> Option<Transition<'a>> {
        let cond_id = eff.annotations.transition?;
        let condition = self
            .condition(cond_id)
            .unwrap_or_else(|| panic!("Transition annotation refers to an absent condition: {cond_id:?}"));
        Some(self.annotated_transition(condition, eff, env))
    }

    /// All transitions annotated in the action.
    pub fn transitions<'a>(&'a self, env: &'a Environment) -> impl Iterator<Item = Transition<'a>> + 'a {
        (self.conditions.iter()).filter_map(move |c| self.transition_of_condition(c, env))
    }

    /// The transition formed by an annotated pair, that is trusted to be valid (checked in debug builds only).
    fn annotated_transition<'a>(&self, cond: &'a Condition, eff: &'a Effect, env: &Environment) -> Transition<'a> {
        if cfg!(debug_assertions) {
            assert!(
                self.conditions.iter().any(|c| std::ptr::eq(c, cond)) && self.effects.iter().any(|e| std::ptr::eq(e, eff)),
                "Transition of an element that is not part of the action"
            );
            assert!(
                cond.annotations.transition == Some(eff.id) && eff.annotations.transition == Some(cond.id),
                "Transition annotation is not symmetric: {cond:?} / {eff:?}"
            );
            check_transition(self, cond, eff, env).unwrap_or_else(|| {
                panic!(
                    "Invalid transition annotation in action {}: {} / {}",
                    self.name,
                    env / cond,
                    env / eff
                )
            })
        } else {
            transition_parts(cond, eff, env).expect("Invalid transition annotation")
        }
    }
}

/// Returns the transition formed by the condition and effect of the action, if:
///  - the condition is an atom on a state variable with simple arguments: `sv`, `(not sv)`, `(= sv v)` or `(= v sv)`;
///  - the effect is an unconditional and unquantified assignment to the same state variable;
///  - the condition's interval ends when the effect's transition starts.
///
/// It is a borrow if the required and assigned values are syntactically identical simple values.
pub(crate) fn check_transition<'a>(
    action: &Action,
    condition: &'a Condition,
    effect: &'a Effect,
    env: &Environment,
) -> Option<Transition<'a>> {
    let e = &effect.effect_expression;
    if e.condition.is_some() || !effect.universal_quantification.is_empty() {
        return None;
    }
    let (csv, _) = required_value(condition.cond, env)?;
    let csv = state_variable(csv, env)?;
    let esv = (e.state_variable.fluent, Term::all_of(&e.state_variable.arguments, env)?);
    if csv != esv || !simultaneous(action, condition.interval.end, e.transition_start) {
        return None;
    }
    transition_parts(condition, effect, env)
}

/// Builds the transition of a condition and an assignment, *assuming* they form a transition.
/// Returns `None` if the condition is not on a state variable or the effect not an assignment.
fn transition_parts<'a>(condition: &'a Condition, effect: &'a Effect, env: &Environment) -> Option<Transition<'a>> {
    let (_, required) = required_value(condition.cond, env)?;
    let EffectOp::Assign(value) = effect.effect_expression.operation else {
        return None;
    };
    let required_simple = match required {
        RequiredValue::Bool(b) => Some(Value::Bool(b)),
        RequiredValue::Expr(e) => Value::of(e, env),
    };
    let borrow = required_simple.is_some() && required_simple == Value::of(value, env);
    Some(Transition {
        condition,
        effect,
        required,
        value,
        borrow,
    })
}

/// True if the two timepoints of the action are the same instant in any instance of the action.
fn simultaneous(action: &Action, t1: Timestamp, t2: Timestamp) -> bool {
    let instantaneous = match action.duration {
        Duration::Instantaneous => true,
        Duration::Fixed(_) | Duration::Bounded(..) | Duration::Subtasks => false,
    };
    // in an instantaneous action, the end is the start
    let normalize = |t: Timestamp| match t.reference {
        TimeRef::ActionEnd if instantaneous => Timestamp {
            reference: TimeRef::ActionStart,
            ..t
        },
        _ => t,
    };
    normalize(t1) == normalize(t2)
}

/// The value required by a condition on a state variable.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RequiredValue {
    /// Implicit value of a boolean atom: `true` for `sv` and `false` for `(not sv)`.
    Bool(bool),
    /// Explicit value `v` of an equality `(= sv v)` or `(= v sv)`.
    Expr(ExprId),
}

/// If the condition is of the form `sv`, `(not sv)`, `(= sv v)` or `(= v sv)`, returns the state variable and the
/// required value.
fn required_value(cond: ExprId, env: &Environment) -> Option<(ExprId, RequiredValue)> {
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

fn state_variable(expr: ExprId, env: &Environment) -> Option<(FluentId, Vec<Term>)> {
    match env.node(expr).expr() {
        Expr::StateVariable(fluent, args) => Some((*fluent, Term::all_of(args, env)?)),
        _ => None,
    }
}

/// A simple value, that can be syntactically compared.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Bool(bool),
    Real(RealValue),
    Term(Term),
}

impl Value {
    fn of(expr: ExprId, env: &Environment) -> Option<Value> {
        match env.node(expr).expr() {
            Expr::Bool(b) => Some(Value::Bool(*b)),
            Expr::Real(r) => Some(Value::Real(*r)),
            _ => Term::of(expr, env).map(Value::Term),
        }
    }
}

/// Displays a transition (or borrow) in a compact form:
///
/// ```text
///   [start, end..] at(?r) : ?l1 =~> ?l2         (transition)
///   [start, end..] available(?x) =~= true       (borrow)
///   [start..end, end..] p(?x) : true =~> false  (durative condition)
/// ```
///
/// The interval is `[<condition>, <end of the effect's transition>]` where the condition is either an instant or a
/// span `<start>..<end>` (an open start is denoted by `(`) and `t..` stands for `t+dt`.
/// If both the condition and the effect are instantaneous, it is abbreviated to `[t..]`.
impl<'env> std::fmt::Display for Env<'env, Transition<'_>> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Transition {
            condition: cond,
            effect: eff,
            required,
            value,
            borrow,
        } = self.elem;
        let eff = &eff.effect_expression;

        // temporal qualifier
        let itv = cond.interval;
        let ts = eff.transition_start;
        let te = eff.transition_end;
        let fmt_end = |t: Timestamp| {
            if t.num_dt == 1 {
                format!("{}..", t - Dt)
            } else {
                t.to_string()
            }
        };
        if itv.start == itv.end && te == ts + Dt {
            write!(f, "[{ts}..] ")?;
        } else if itv.start == itv.end {
            write!(f, "[{}, {}] ", itv.end, fmt_end(te))?;
        } else if itv.start.num_dt == 1 {
            write!(f, "({}..{}, {}] ", itv.start - Dt, itv.end, fmt_end(te))?;
        } else {
            write!(f, "[{}..{}, {}] ", itv.start, itv.end, fmt_end(te))?;
        }

        write!(f, "{}", self.env / &eff.state_variable)?;
        if borrow {
            write!(f, " =~= {}", self.env / value)
        } else {
            match required {
                RequiredValue::Bool(b) => write!(f, " : {b}")?,
                RequiredValue::Expr(e) => write!(f, " : {}", self.env / e)?,
            }
            write!(f, " =~> {}", self.env / value)
        }
    }
}
