pub mod interfaces;
mod macros;
mod models;
mod procedures;
mod traits;

// Public exportation of the interfaces
pub use interfaces::unified_planning::validate_upf;

use std::{collections::BTreeMap, fmt::Debug};

use anyhow::{bail, Result};
use malachite::Rational;
use models::{
    action::{Action, DurativeAction, SpanAction},
    condition::{Condition, DurativeCondition, SpanCondition},
    env::Env,
};
use traits::interpreter::Interpreter;

use crate::traits::act::Act;

/* ========================================================================== */

/// Validates a plan.
pub fn validate<E: Interpreter + Clone + Debug + std::cmp::PartialEq>(
    env: &mut Env<E>,
    actions: &[Action<E>],
    goals: &[Condition<E>],
    is_temporal: bool,
) -> Result<()> {
    if is_temporal {
        let dur_actions = actions
            .iter()
            .map(|a| match a {
                Action::Span(_) => bail!("Span action found for a temporal plan."),
                Action::Durative(a) => Ok(a.clone()),
            })
            .collect::<Result<Vec<_>>>()?;
        let mut span_goals = vec![];
        let mut dur_goals = vec![];
        for goal in goals.iter() {
            match goal {
                Condition::Span(g) => span_goals.push(g.clone()),
                Condition::Durative(g) => dur_goals.push(g.clone()),
            };
        }
        validate_temporal(env, &dur_actions, &span_goals, &dur_goals)
    } else {
        let span_actions = actions
            .iter()
            .map(|a| match a {
                Action::Durative(_) => bail!("Durative action found for a nontemporal plan."),
                Action::Span(a) => Ok(a.clone()),
            })
            .collect::<Result<Vec<_>>>()?;
        let span_goals = goals
            .iter()
            .map(|a| match a {
                Condition::Durative(_) => bail!("Durative goal found for a temporal plan."),
                Condition::Span(c) => Ok(c.clone()),
            })
            .collect::<Result<Vec<_>>>()?;
        validate_nontemporal(env, &span_actions, &span_goals)
    }
}

/* ========================================================================== */

/// Validates a non temporal plan.
fn validate_nontemporal<E: Interpreter + Clone + Debug>(
    env: &mut Env<E>,
    actions: &[SpanAction<E>],
    goals: &[SpanCondition<E>],
) -> Result<()> {
    print_info!(env.verbose, "Simulation of the plan");
    for a in actions {
        print_info!(env.verbose, "Action {}", a.name());
        if let Some(s) = a.apply(env, env.state())? {
            env.set_state(s);
        } else {
            bail!("Non applicable action {a:?}");
        }
    }

    print_info!(env.verbose, "Check the goal has been reached");
    for g in goals {
        if !g.is_valid(env)? {
            bail!("Unreached goal {g:?}");
        }
    }

    print_info!(env.verbose, "The plan is valid");
    Ok(())
}

/* ========================================================================== */

/// Validates a temporal plan.
fn validate_temporal<E: Interpreter + Clone + Debug + std::cmp::PartialEq>(
    env: &mut Env<E>,
    actions: &[DurativeAction<E>],
    span_goals: &[SpanCondition<E>],
    dur_goals: &[DurativeCondition<E>],
) -> Result<()> {
    /// Returns the name of the new action for the given timepoint.
    fn action_name(t: &Rational) -> String {
        format!("action_{t}")
    }

    /// Adds the start and end timepoints of the condition.
    fn add_condition_terminal<E: Interpreter + Clone>(
        condition: &DurativeCondition<E>,
        action: Option<&DurativeAction<E>>,
        global_end: &Rational,
        epsilon: &Rational,
        span_actions_map: &mut BTreeMap<Rational, SpanAction<E>>,
    ) {
        let mut start = condition.interval().start().eval(action, global_end);
        if condition.interval().is_start_open() {
            start += epsilon.clone();
        }
        let mut end = condition.interval().end().eval(action, global_end);
        if condition.interval().is_end_open() {
            end -= epsilon.clone();
        }

        let mut set_action = |t: Rational| {
            span_actions_map
                .entry(t.clone())
                .and_modify(|a| a.add_condition(condition.to_span().clone()))
                .or_insert_with(|| {
                    SpanAction::new(
                        action_name(&t),
                        action_name(&t),
                        vec![condition.to_span().clone()],
                        vec![],
                    )
                });
        };
        set_action(start);
        set_action(end);
    }

    /*=================================================================*/

    print_info!(
        env.verbose,
        "Group the effects/conditions by timepoints in span actions"
    );
    // Get the plan duration.
    let mut global_end = Rational::from(0);
    for action in actions {
        let action_end = action.end().eval(Some(action), &global_end);
        if action_end > global_end {
            global_end = action_end;
        }
    }

    // Group the effects by timepoints.
    let mut span_actions_map = BTreeMap::<Rational, SpanAction<E>>::new();
    for action in actions {
        for effect in action.effects() {
            let t = effect.occurrence().eval(Some(action), &global_end);
            print_info!(env.verbose, "Timepoint {t}");
            print_info!(env.verbose, "Effect {effect:?}");
            span_actions_map
                .entry(t.clone())
                .and_modify(|a| a.add_effect(effect.to_span().clone()))
                .or_insert_with(|| {
                    SpanAction::new(action_name(&t), action_name(&t), vec![], vec![effect.to_span().clone()])
                });
        }
    }

    // Calculate epsilon
    let mut epsilon = Rational::from(i64::MAX);
    let mut prev_timepoint = None;
    for (timepoint, _) in span_actions_map.iter() {
        if let Some(prev_timepoint) = prev_timepoint {
            let diff = timepoint.clone() - prev_timepoint;
            if diff < epsilon {
                epsilon = diff;
            }
        }
        prev_timepoint = Some(timepoint);
    }
    epsilon /= Rational::from(10);

    // Add the conditions start and end timepoints.
    for action in actions {
        for condition in action.conditions() {
            add_condition_terminal(condition, Some(action), &global_end, &epsilon, &mut span_actions_map);
        }
    }

    // Add the durative goals start and end timepoints.
    for goal in dur_goals {
        add_condition_terminal(goal, None, &global_end, &epsilon, &mut span_actions_map);
    }

    // Add the conditions and durative goals into every timepoints of their interval.
    // Notes: Will be duplicated into start and end timepoints, but it is not a problem.
    for (timepoint, span_action) in span_actions_map.iter_mut() {
        for action in actions {
            for condition in action.conditions() {
                if condition.interval().contains(timepoint, Some(action), &global_end) {
                    span_action.add_condition(condition.to_span().clone());
                }
            }
        }

        for goal in dur_goals {
            if goal.interval().contains::<E>(timepoint, None, &global_end) {
                span_action.add_condition(goal.to_span().clone());
            }
        }
    }

    // Extract span actions from the map.
    let mut span_actions = vec![];
    for (_, span_action) in span_actions_map.iter() {
        span_actions.push(span_action.clone());
    }

    // Validation.
    validate_nontemporal(env, &span_actions, span_goals)
}
