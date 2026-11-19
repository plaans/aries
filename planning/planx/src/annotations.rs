//! Annotations attached to the conditions and effects of actions.
//!
//! Annotations record information on the role of a condition or effect in its action (e.g. being part of a
//! transition), typically computed by a preprocessing step to be exploited by later ones or by the encoding.
//! Each kind of annotation is a field of [`FluentAnnotations`], [`ActionAnnotations`], [`ConditionAnnotations`] or
//! [`EffectAnnotations`].
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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use itertools::Itertools;

use crate::preprocessing::analysis::{ActionTiming, RequiredValue, Term, required_value, requires_value};
use crate::*;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

static COLORED: AtomicBool = AtomicBool::new(false);

/// Enables or disables the use of ANSI colors when displaying annotations (disabled by default).
/// Typically enabled when printing to a terminal.
pub fn set_colored_annotations(colored: bool) {
    COLORED.store(colored, Ordering::Relaxed);
}

/// Displays its content in grey (if colors are enabled), to make annotations visually discrete.
pub struct Discrete<T>(pub T);

impl<T: std::fmt::Display> std::fmt::Display for Discrete<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if COLORED.load(Ordering::Relaxed) {
            write!(f, "\x1b[90m{}\x1b[0m", self.0)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

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

/// Identifier of a [`Goal`], unique within its model.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GoalId(u64);

impl GoalId {
    /// A new identifier, distinct from all previously created ones.
    pub fn fresh() -> Self {
        GoalId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for GoalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "g{}", self.0)
    }
}

impl std::fmt::Display for ConditionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.0)
    }
}

impl std::fmt::Display for EffectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "e{}", self.0)
    }
}

impl std::fmt::Display for CondRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.action, self.condition)
    }
}

impl std::fmt::Display for EffRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.action, self.effect)
    }
}

/// Reference to a condition of an action of the model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CondRef {
    pub action: ActionRef,
    pub condition: ConditionId,
}

/// Reference to an effect of an action of the model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffRef {
    pub action: ActionRef,
    pub effect: EffectId,
}

/// Annotations of a fluent (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct FluentAnnotations {
    /// Properties of the fluent (see [`annotate_fluents`](crate::preprocessing::annotate_fluents)).
    /// `None` if not computed.
    pub properties: Option<FluentProperties>,
}

/// Properties of a fluent, derived from the way it is used in the model (and, for static fluents, from its initial
/// values).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FluentProperties {
    /// True if the fluent is never the target of an action effect: its value is entirely determined by the problem
    /// definition.
    pub is_static: bool,
    /// True if the fluent is read somewhere in the model (condition, goal, metric, effect argument or value, ...).
    pub read: bool,
    /// True if the fluent is read somewhere else than in a *simple requirement*, i.e. a top-level conjunct `sv`,
    /// `(not sv)`, `(= sv v)` of an action condition or of an unquantified goal, where all arguments of `sv` and the
    /// value `v` are parameters or constants.
    ///
    /// Reasoning on which effect can support which requirement is only possible for non-opaque fluents.
    pub opaque: bool,
    /// For a static boolean binary relation, the positions of the arguments that functionally determine the other
    /// one (see [`StaticFunctions`](crate::preprocessing::analysis::StaticFunctions)).
    pub functional_inputs: Vec<usize>,
    /// For a static boolean binary relation whose two arguments have the same type: `Some(n)` if the relation
    /// (as a graph over objects) has no cycle, `n` being the number of edges of its longest path.
    pub acyclic: Option<usize>,
    /// For a numeric fluent, the impact of increasing or decreasing it.
    pub numeric: Option<NumericProperties>,
}

/// Impact of changing a numeric fluent in a given direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DirectionImpact {
    /// The fluent is never read: changing it has no impact.
    Neutral,
    /// Changing the fluent in this direction can only make requirements easier to satisfy and the metric better.
    Helpful,
    /// Changing the fluent in this direction can only make requirements harder to satisfy and the metric worse.
    Harmful,
    /// Unknown or both helpful and harmful.
    Mixed,
}

/// Properties of a numeric fluent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumericProperties {
    /// Impact of increasing the fluent.
    pub increase: DirectionImpact,
    /// Impact of decreasing the fluent.
    pub decrease: DirectionImpact,
    /// True if no action effect increases the fluent (decrease or assignment of a smaller constant are allowed).
    pub never_increased: bool,
    /// True if no action effect decreases the fluent.
    pub never_decreased: bool,
}

/// Annotations of a goal (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct GoalAnnotations {
    /// For a goal made of simple requirements, the elements that may support each of them (in the order of the
    /// conjuncts of the goal expression), see [`annotate_supports`](crate::preprocessing::annotate_supports).
    /// `None` if not computed or if the goal is not made of simple requirements.
    pub supporters: Option<Vec<Supporters>>,
}

/// Annotations of an action (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct ActionAnnotations {
    /// Reachability and usefulness of the action (see [`annotate_supports`](crate::preprocessing::annotate_supports)).
    /// `None` if not computed.
    pub status: Option<ActionStatus>,
}

/// Reachability and usefulness of an action.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ActionStatus {
    /// False if the action cannot appear in any valid plan, because some of its requirements cannot be supported.
    pub reachable: bool,
    /// False if the action cannot contribute to the goals or metric, even indirectly: it does not appear in any
    /// *non-redundant* plan (plan in which each action contributes to the goals or metric).
    pub useful: bool,
}

/// Annotations of a condition of an action (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct ConditionAnnotations {
    /// Effect forming a transition with this condition (see [`annotate_transitions`](crate::preprocessing::annotate_transitions)).
    /// Only read through [`Action::transition_of_condition`], which checks that it is still valid.
    pub(crate) transition: Option<EffectId>,
    /// Elements that may support the condition (see [`annotate_supports`](crate::preprocessing::annotate_supports)).
    /// `None` if not computed or if the condition is not a simple requirement.
    pub supporters: Option<Supporters>,
}

/// Annotations of an effect of an action (one field per kind of annotation).
#[derive(Clone, Debug, Default)]
pub struct EffectAnnotations {
    /// Condition forming a transition with this effect (see [`annotate_transitions`](crate::preprocessing::annotate_transitions)).
    /// Only read through [`Action::transition_of_effect`], which checks that it is still valid.
    pub(crate) transition: Option<ConditionId>,
    /// Requirements the effect may support (see [`annotate_supports`](crate::preprocessing::annotate_supports)).
    /// `None` if not computed.
    pub supports: Option<Supports>,
}

/// Requirements that an effect may support, i.e. the requirements of the model it may be the reason of an action for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Supports {
    /// The effect is on a fluent that is not only read in simple requirements: it may be useful for anything.
    Opaque,
    /// The effect may only support these requirements (an empty list means that the effect is never useful).
    Targets(Vec<SupportTarget>),
}

impl Supports {
    /// True if the effect cannot support anything.
    pub fn is_useless(&self) -> bool {
        matches!(self, Supports::Targets(ts) if ts.is_empty())
    }
}

/// A requirement that an effect may support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupportTarget {
    /// A condition of an action.
    ///
    /// `undo` is true if the condition is part of a transition that (when supported by the effect) restores the value
    /// that the effect's own transition consumed: supporting it only makes sense if the two actions cancel each other.
    Condition { cond: CondRef, undo: bool },
    /// A goal of the model. `atoms` is the number of atoms of the goal that the effect may support.
    Goal { goal: GoalId, atoms: usize },
}

/// Elements that may support a condition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Supporters {
    /// True if the condition may be supported by the problem definition (initial state or timed effects).
    pub init: bool,
    /// Effects of actions that may support the condition.
    pub effects: Vec<EffRef>,
}

/// A condition immediately followed by an assignment (or erasure) of the same state variable.
#[derive(Copy, Clone, Debug)]
pub struct Transition<'a> {
    pub condition: &'a Condition,
    pub effect: &'a Effect,
    /// The value required by the condition.
    pub required: RequiredValue,
    /// The value assigned by the effect, or `None` if the effect is an erasure.
    pub value: Option<ExprId>,
    /// True if the effect establishes the value required by the condition.
    pub borrow: bool,
}

impl Action {
    /// True if the condition is consumed by the action: it is the condition of a transition that is not a borrow, so
    /// that the value it requires is replaced by the action just after being required.
    pub fn is_consuming(&self, cond: &Condition, env: &Environment) -> bool {
        self.transition_of_condition(cond, env).is_some_and(|t| !t.borrow)
    }

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
                self.conditions.iter().any(|c| std::ptr::eq(c, cond))
                    && self.effects.iter().any(|e| std::ptr::eq(e, eff)),
                "Transition of an element that is not part of the action"
            );
            assert!(
                cond.annotations.transition == Some(eff.id) && eff.annotations.transition == Some(cond.id),
                "Transition annotation is not symmetric: {cond:?} / {eff:?}"
            );
            let transition = (transition_parts(cond, eff, env))
                .filter(|_| {
                    same_state_variable(cond, eff, env)
                        && !provably_distinct(self, cond.interval.end, eff.effect_expression.transition_start, env)
                })
                .filter(|t| t.shape_ok());
            transition.unwrap_or_else(|| {
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
///  - the effect is an unconditional and unquantified assignment or erasure of the same state variable;
///  - the condition's interval ends when the effect's transition starts, in any instance of the action.
///
/// It is a borrow if the required and assigned values are syntactically identical simple values.
pub(crate) fn check_transition<'a>(
    timing: &ActionTiming,
    condition: &'a Condition,
    effect: &'a Effect,
    env: &Environment,
) -> Option<Transition<'a>> {
    if !same_state_variable(condition, effect, env)
        || !timing.eq(condition.interval.end, effect.effect_expression.transition_start)
    {
        return None;
    }
    transition_parts(condition, effect, env).filter(|t| t.shape_ok())
}

impl Transition<'_> {
    /// Checks that a borrow can be encoded as a condition over the whole borrow, together with an exclusion of all
    /// other conditions on the state variable (instead of an assignment effect).
    /// Returns the reason why it cannot otherwise.
    ///
    /// This requires that:
    ///  - the effect has a non-instantaneous transition (`start < end`), so that the condition can be extended to `end - ε`;
    ///  - the state variable is not numeric (increase and decrease effects rely on the window of an assignment);
    ///  - no other effect of the action on the fluent (with arguments that may unify) may start during the transition
    ///    `[start, end]` of the borrow's effect (it would be cancelled by, or cancel, the borrow's effect under the
    ///    PDDL add-after-delete semantics, or be excluded by its window).
    ///
    /// Only borrows satisfying these requirements are annotated by
    /// [`annotate_transitions`](crate::preprocessing::annotate_transitions).
    pub fn borrow_requirements(&self, action: &Action, timing: &ActionTiming, env: &Environment) -> Result<(), &'static str> {
        let e = &self.effect.effect_expression;
        let sv = &e.state_variable;
        let may_unify = |a: &[ExprId], b: &[ExprId]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(&x, &y)| match (Term::of(x, env), Term::of(y, env)) {
                    (Some(x), Some(y)) => x.may_unify(&y),
                    _ => true,
                })
        };
        if !self.borrow {
            Err("not a borrow")
        } else if !self.shape_ok() {
            Err("conditional or quantified effect")
        } else if !timing.lt(e.transition_start, e.transition_end) {
            Err("possibly instantaneous transition")
        } else if env.fluents.get(sv.fluent).return_type.is_numeric() {
            Err("numeric state variable")
        } else if action.effects.iter().any(|other| {
            let o = &other.effect_expression;
            !std::ptr::eq(other, self.effect)
                && o.state_variable.fluent == sv.fluent
                && may_unify(&o.state_variable.arguments, &sv.arguments)
                && !timing.lt(o.transition_start, e.transition_start)
                && !timing.lt(e.transition_end, o.transition_start)
        }) {
            Err("another effect of the action on the state variable may start during the borrow")
        } else {
            Ok(())
        }
    }

    /// True if the effect is unconditional and unquantified.
    fn shape_ok(&self) -> bool {
        self.effect.effect_expression.condition.is_none() && self.effect.universal_quantification.is_empty()
    }
}

/// True if the condition is on the state variable (with simple arguments) affected by the effect.
fn same_state_variable(condition: &Condition, effect: &Effect, env: &Environment) -> bool {
    let sv = |e: ExprId| match env.node(e).expr() {
        Expr::StateVariable(fluent, args) => Some((*fluent, Term::all_of(args, env)?)),
        _ => None,
    };
    let esv = &effect.effect_expression.state_variable;
    let esv = Term::all_of(&esv.arguments, env).map(|args| (esv.fluent, args));
    esv.is_some() && required_value(condition.cond, env).and_then(|(csv, _)| sv(csv)) == esv
}

/// True if the two timestamps of the action are provably distinct, whatever the value of `dt`.
///
/// Used to check transitions in debug builds, where the bounds on the duration of the action are not available
/// (only a constant duration is exploited): this may miss some invalid transitions but never rejects a valid one.
fn provably_distinct(action: &Action, t1: Timestamp, t2: Timestamp, env: &Environment) -> bool {
    let duration = match action.duration {
        Duration::Instantaneous => Some(RealValue::ZERO),
        Duration::Fixed(d) => match env.node(d).expr() {
            Expr::Real(d) => Some(*d),
            _ => None,
        },
        Duration::Bounded(..) | Duration::Subtasks => None,
    };
    // t2 - t1 = offset + num_dt * dt
    let delay = t2.delay - t1.delay;
    let offset = match (t1.reference, t2.reference) {
        (r1, r2) if r1 == r2 => Some(delay),
        (TimeRef::ActionStart, TimeRef::ActionEnd) => duration.map(|d| delay + d),
        (TimeRef::ActionEnd, TimeRef::ActionStart) => duration.map(|d| delay - d),
        _ => None,
    };
    let Some(offset) = offset else {
        return false;
    };
    let num_dt = t2.num_dt - t1.num_dt;
    let zero = RealValue::ZERO;
    // the two terms cannot cancel each other for any `dt > 0`
    (offset == zero) != (num_dt == 0) || (offset > zero && num_dt > 0) || (offset < zero && num_dt < 0)
}

/// Builds the transition of a condition and an effect, *assuming* they form a transition.
/// Returns `None` if the condition is not on a state variable or the effect neither an assignment nor an erasure.
fn transition_parts<'a>(condition: &'a Condition, effect: &'a Effect, env: &Environment) -> Option<Transition<'a>> {
    let (_, required) = required_value(condition.cond, env)?;
    let value = match effect.effect_expression.operation {
        EffectOp::Assign(value) => Some(value),
        EffectOp::Erase => None,
        EffectOp::Increase(_) | EffectOp::Decrease(_) => return None,
    };
    let borrow = value.is_some_and(|v| requires_value(required, v, env));
    Some(Transition {
        condition,
        effect,
        required,
        value,
        borrow,
    })
}

/// Displays a transition (or borrow) in a compact form:
///
/// ```text
///   [start, end..] at(?r) : ?l1 =~> ?l2         (transition)
///   [start, end..] available(?x) =~= true       (borrow)
///   [start..end, end..] p(?x) : true =~> false  (durative condition)
///   [start..] p(?x) : true =~> ∅                (erasure)
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
        match value {
            Some(value) if borrow => write!(f, " =~= {}", self.env / value),
            _ => {
                match required {
                    RequiredValue::Bool(b) => write!(f, " : {b}")?,
                    RequiredValue::Expr(e) => write!(f, " : {}", self.env / e)?,
                }
                match value {
                    Some(value) => write!(f, " =~> {}", self.env / value),
                    None => write!(f, " =~> ∅"),
                }
            }
        }
    }
}

impl std::fmt::Display for FluentProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut props = Vec::new();
        if self.is_static {
            props.push("static".to_string());
        }
        if !self.read {
            props.push("unread".to_string());
        } else if self.opaque {
            props.push("opaque".to_string());
        }
        if !self.functional_inputs.is_empty() {
            props.push(format!("functional({})", self.functional_inputs.iter().join(",")));
        }
        if let Some(n) = self.acyclic {
            props.push(format!("acyclic(longest path: {n})"));
        }
        if let Some(num) = &self.numeric {
            props.push(format!("increase: {:?}, decrease: {:?}", num.increase, num.decrease).to_lowercase());
            if num.never_increased {
                props.push("never-increased".to_string());
            }
            if num.never_decreased {
                props.push("never-decreased".to_string());
            }
        }
        write!(f, "[{}]", props.join(", "))
    }
}

impl std::fmt::Display for SupportTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupportTarget::Condition { cond, undo: false } => write!(f, "{cond}"),
            SupportTarget::Condition { cond, undo: true } => write!(f, "{cond}(undo)"),
            SupportTarget::Goal { goal, atoms: 1 } => write!(f, "{goal}"),
            SupportTarget::Goal { goal, atoms } => write!(f, "{goal}(x{atoms})"),
        }
    }
}

impl std::fmt::Display for Supports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Supports::Opaque => write!(f, "<opaque>"),
            Supports::Targets(ts) if ts.is_empty() => write!(f, "<nothing>"),
            Supports::Targets(ts) => write!(f, "{}", ts.iter().join(", ")),
        }
    }
}

impl std::fmt::Display for Supporters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let init = self.init.then(|| "init".to_string());
        let effects = self.effects.iter().map(|e| e.to_string());
        let all = init.into_iter().chain(effects).collect_vec();
        if all.is_empty() {
            write!(f, "<nothing>")
        } else {
            write!(f, "{}", all.join(", "))
        }
    }
}
