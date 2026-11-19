//! Generators based on numeric fluents used as consumable resources.

use std::collections::HashMap;

use crate::preprocessing::analysis::{ActionTiming, NumericBounds, Term, conjuncts};
use crate::*;

use super::{Cmp, Constraint, CountConstraint, Linear, PlanScope};

pub(super) fn generate(model: &Model, out: &mut Vec<CountConstraint>) {
    let bounds = NumericBounds::new(model);
    for (id, fluent) in model.env.fluents.iter_with_id() {
        let Some(num) = fluent.annotations.properties.as_ref().and_then(|p| p.numeric.as_ref()) else {
            continue;
        };
        if num.never_increased && !num.never_decreased {
            budget(model, id, Direction::Decrease, &bounds, out);
        } else if num.never_decreased && !num.never_increased {
            budget(model, id, Direction::Increase, &bounds, out);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Direction {
    Increase,
    Decrease,
}

fn floor(x: RealValue) -> i64 {
    x.numer().div_euclid(x.denom())
}

/// A fluent that is only decreased (resp. increased) and whose every change is guarded by a condition `f >= g`
/// (resp. `f <= g`) just before it: after any change, `f >= lb(g) - ub(delta)` (resp. `f <= ub(g) + ub(delta)`).
/// Hence the total amount of change of a state variable is bounded by the distance between its initial value and this
/// threshold.
fn budget(model: &Model, fluent: FluentId, dir: Direction, bounds: &NumericBounds, out: &mut Vec<CountConstraint>) {
    let env = &model.env;
    let name = (env / fluent).name();
    // threshold such that `f >= threshold` (decrease) or `-f >= threshold` (increase) after any change
    let mut threshold: Option<RealValue> = None;
    let mut terms = Vec::new();
    for a in model.actions.iter() {
        let timing = ActionTiming::new(a, bounds);
        let mut coef = RealValue::ZERO;
        for e in &a.effects {
            let ee = &e.effect_expression;
            if ee.state_variable.fluent != fluent {
                continue;
            }
            let delta = match (ee.operation.clone(), dir) {
                (EffectOp::Decrease(d), Direction::Decrease) | (EffectOp::Increase(d), Direction::Increase) => d,
                _ => return, // not a monotone change (assignments are excluded)
            };
            let b = bounds.bounds(delta);
            let (Some(dlb), Some(dub)) = (b.lb, b.ub) else { return };
            if ee.condition.is_some() || !e.universal_quantification.is_empty() {
                return;
            }
            let Some(args) = Term::all_of(&ee.state_variable.arguments, env) else {
                return;
            };
            // guard: best bound on the value just before the change
            let mut guard: Option<RealValue> = None;
            for c in &a.conditions {
                if !timing.holds_before(c.interval, ee.transition_start) {
                    continue;
                }
                for conj in conjuncts(c.cond, env) {
                    if let Some(g) = guard_bound(conj, fluent, &args, dir, env, bounds) {
                        guard = Some(guard.map_or(g, |x: RealValue| x.max(g)));
                    }
                }
            }
            let Some(g) = guard else { return };
            let after = g - dub;
            threshold = Some(threshold.map_or(after, |t: RealValue| t.min(after)));
            coef = coef + dlb;
        }
        let c = floor(coef);
        if c > 0 {
            terms.push((c, a.name.clone()));
        }
    }
    let Some(threshold) = threshold else { return };
    if terms.is_empty() {
        return;
    }
    // initial values
    let mut initial: HashMap<Vec<Term>, RealValue> = HashMap::new();
    for e in &model.init {
        let ee = &e.effect_expression;
        if ee.state_variable.fluent != fluent {
            continue;
        }
        let EffectOp::Assign(v) = ee.operation else { return };
        let (Expr::Real(v), Some(args)) = (
            env.node(v).expr().clone(),
            Term::all_of(&ee.state_variable.arguments, env),
        ) else {
            return;
        };
        if ee.transition_start != Timestamp::ORIGIN {
            return;
        }
        let v = if dir == Direction::Decrease { v } else { -v };
        initial.insert(args, v);
    }
    let total: RealValue = (initial.values())
        .map(|&v| if v > threshold { v - threshold } else { RealValue::ZERO })
        .fold(RealValue::ZERO, |a, b| a + b);
    let verb = if dir == Direction::Decrease {
        "decreased"
    } else {
        "increased"
    };
    out.push(CountConstraint {
        constraint: Constraint::Linear(Linear::new(terms, Cmp::Le, floor(total))),
        scope: PlanScope::AllPlans,
        generator: "numeric-budget",
        explanation: format!(
            "`{name}` is only {verb}, each change being guarded: the total change over the {} initialized state variable(s) is at most {total} (coefficients: minimal change of each action)",
            initial.len()
        ),
    });
}

/// If the expression is a guard on the state variable `fluent(args)`, returns the bound `g` such that the guard
/// implies `f >= g` (decrease) or `-f >= g` (increase).
fn guard_bound(
    expr: ExprId,
    fluent: FluentId,
    args: &[Term],
    dir: Direction,
    env: &Environment,
    bounds: &NumericBounds,
) -> Option<RealValue> {
    let is_sv = |e: ExprId| match env.node(e).expr() {
        Expr::StateVariable(f, xs) => *f == fluent && Term::all_of(xs, env).as_deref() == Some(args),
        _ => false,
    };
    let Expr::App(fun, xs) = env.node(expr).expr().clone() else {
        return None;
    };
    let [x, y] = xs.as_slice() else { return None };
    // normalize to `lhs >= rhs` (strict comparisons are weakened)
    let (lhs, rhs) = match fun {
        Fun::Geq | Fun::Gt => (*x, *y),
        Fun::Leq | Fun::Lt => (*y, *x),
        _ => return None,
    };
    match dir {
        // f >= g
        Direction::Decrease if is_sv(lhs) => bounds.bounds(rhs).lb,
        // g >= f  i.e.  -f >= -g
        Direction::Increase if is_sv(rhs) => bounds.bounds(lhs).ub.map(|u| -u),
        _ => None,
    }
}
