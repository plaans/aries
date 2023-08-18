pub mod interfaces;
mod macros;
mod models;
mod procedures;
mod traits;

use aries::core::INT_CST_MAX;
// Public exportation of the interfaces
pub use interfaces::unified_planning::validate_upf;

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use anyhow::{bail, ensure, Result};
use malachite::{num::arithmetic::traits::Abs, Rational};
use models::{
    action::{Action, DurativeAction, SpanAction},
    condition::{Condition, DurativeCondition, SpanCondition},
    env::Env,
    state::State,
    task::Task,
};
use traits::interpreter::Interpreter;

use crate::{
    models::{
        csp::{CspConstraint, CspConstraintTerm, CspProblem, CspVariable},
        method::Method,
    },
    traits::{act::Act, configurable::Configurable, durative::Durative},
};

const EMPTY_ACTION: &str = "__empty_action__";

/* ========================================================================== */
/*                               Entry Function                               */
/* ========================================================================== */

/// Validates a plan.
pub fn validate<E: Interpreter + Clone + Display>(
    env: &mut Env<E>,
    actions: &[Action<E>],
    root_tasks: Option<&HashMap<String, Task<E>>>,
    goals: &[Condition<E>],
    is_temporal: bool,
    min_epsilon: &Option<Rational>,
) -> Result<()> {
    /* =================== Plan Analyze Without Hierarchy =================== */
    let states = if is_temporal {
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
        validate_temporal(env, &dur_actions, &span_goals, &dur_goals, min_epsilon)?
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

        let mut c = 0;
        env.epsilon = 1.into();
        validate_nontemporal(env, &span_actions, &span_goals)?
            .into_iter()
            .map(|s| {
                c += 2;
                env.global_end = (c - 1).into();
                (Rational::from(c - 2), s)
            })
            .collect()
    };

    /* ========================== Hierarchy Analyze ========================= */
    if let Some(root_task) = root_tasks {
        validate_hierarchy(env, Action::into_durative(actions).as_ref(), root_task, &states)?;
    }
    Ok(())
}

/* ========================================================================== */
/*                              Non Hierarchical                              */
/*                                Non Temporal                                */
/* ========================================================================== */

/// Validates a non temporal plan.
fn validate_nontemporal<E: Interpreter + Clone + Display>(
    env: &mut Env<E>,
    actions: &[SpanAction<E>],
    goals: &[SpanCondition<E>],
) -> Result<Vec<State>> {
    let mut states = vec![env.state().clone()];

    print_info!(env.verbose, "Simulation of the plan");
    for a in actions {
        print_info!(env.verbose, "Action {}", a.name());
        if let Some(s) = a.apply(env, env.state())? {
            env.set_state(s);
            states.push(env.state().clone());
        } else {
            bail!("Non applicable action {}", a.name());
        }
    }

    print_info!(env.verbose, "Check the goal has been reached");
    for g in goals {
        if !g.is_valid(env)? {
            bail!("Unreached goal {g}");
        }
    }

    print_info!(env.verbose, "The plan is valid");
    Ok(states)
}

/* ========================================================================== */
/*                              Non Hierarchical                              */
/*                                  Temporal                                  */
/* ========================================================================== */

/// Validates a temporal plan.
fn validate_temporal<E: Interpreter + Clone + Display>(
    env: &mut Env<E>,
    actions: &[DurativeAction<E>],
    span_goals: &[SpanCondition<E>],
    dur_goals: &[DurativeCondition<E>],
    min_epsilon: &Option<Rational>,
) -> Result<BTreeMap<Rational, State>> {
    /* =========================== Utils Functions ========================== */

    /// Returns the name of the new action for the given timepoint.
    fn action_name(t: &Rational) -> String {
        format!("action_{t}")
    }

    /// Returns the timepoint stored in the action name.
    fn timepoint_from_action_name(n: &String) -> Result<Rational> {
        if n == EMPTY_ACTION {
            return Ok((-1).into());
        }
        let rational_str = n.replace("action_", "");
        let split = rational_str.split('/').collect::<Vec<_>>();
        if split.len() == 1 {
            Ok(Rational::from(rational_str.parse::<i32>()?))
        } else if split.len() == 2 {
            let num = split[0].parse::<i32>()?;
            let den = split[1].parse::<i32>()?;
            Ok(Rational::from_signeds(num, den))
        } else {
            bail!("Malformed action name");
        }
    }

    /// Adds the start and end timepoints of the condition.
    fn add_condition_terminal<E: Interpreter + Clone + Display>(
        condition: &DurativeCondition<E>,
        action: Option<&DurativeAction<E>>,
        env: &Env<E>,
        span_actions_map: &mut BTreeMap<Rational, SpanAction<E>>,
    ) -> Result<()> {
        // Get the timepoints
        let mut start = condition.interval().start(env).eval(action, env);
        if Durative::<E>::is_start_open(condition.interval()) {
            start += env.epsilon.clone();
        }
        let mut end = condition.interval().end(env).eval(action, env);
        if Durative::<E>::is_end_open(condition.interval()) {
            end -= env.epsilon.clone();
        }

        // Check that the timepoints are not too close to others
        let bail = |time: Rational, span_action: &SpanAction<E>| -> Result<()> {
            bail!(
                "Minimal delay of {} between condition ({}) and generated action ({}) is not respected, found {}",
                env.epsilon,
                condition,
                span_action,
                time,
            );
        };
        for (timepoint, action) in span_actions_map.iter() {
            let delta_start = (&start - timepoint).abs();
            let delta_end = (&end - timepoint).abs();

            if delta_start > 0 && delta_start < env.epsilon {
                bail((&start - timepoint).abs(), action)?;
            }
            if delta_end > 0 && delta_end < env.epsilon {
                bail((&end - timepoint).abs(), action)?;
            }
        }

        // Get the parameters
        let params = if let Some(action) = action {
            action.params()
        } else {
            &[]
        };

        // Add the condition to the actions associated with the timepoints
        let mut set_action = |t: Rational| {
            span_actions_map
                .entry(t.clone())
                .and_modify(|a| {
                    a.add_condition(condition.to_span().clone());
                    for p in params {
                        a.add_param(p.clone());
                    }
                })
                .or_insert_with(|| {
                    SpanAction::new(
                        action_name(&t),
                        action_name(&t),
                        params.to_vec(),
                        vec![condition.to_span().clone()],
                        vec![],
                    )
                });
        };
        set_action(start);
        set_action(end);
        Ok(())
    }

    /* ============================ Function Body =========================== */

    print_info!(
        env.verbose,
        "Group the effects/conditions by timepoints in span actions"
    );
    let mut span_actions_map = BTreeMap::<Rational, SpanAction<E>>::new();

    // Get the plan duration, check the duration of the actions and create an empty action for each action start and end.
    env.global_end = Rational::from(0);
    for action in actions {
        let mut new_env = action.new_env_with_params(env);

        // Get the plan duration.
        let action_end = action.end(&new_env).eval(Some(action), &new_env);
        if action_end > env.global_end {
            env.global_end = action_end.clone();
            new_env.global_end = action_end;
        }

        // Check the action duration.
        if let Some(duration) = action.duration().as_ref() {
            let start = action.start(&new_env).eval(Some(action), &new_env);
            let end = action.end(&new_env).eval(Some(action), &new_env);
            let dur = end - start;
            ensure!(
                duration.contains(&new_env, dur.clone())?,
                format!("The actual duration {dur} of the action {action} is not contained in {duration}",)
            );
        } else {
            bail!("Durative action without duration");
        }

        // Create an empty action for start and end.
        let start = action.start(env).eval::<E, DurativeAction<E>>(None, env);
        let end = action.end(env).eval::<E, DurativeAction<E>>(None, env);
        for t in [start, end] {
            span_actions_map
                .entry(t.clone())
                .and_modify(|a| {
                    for p in action.params() {
                        a.add_param(p.clone());
                    }
                })
                .or_insert_with(|| {
                    SpanAction::new(
                        action_name(&t),
                        action_name(&t),
                        action.params().to_vec(),
                        vec![],
                        vec![],
                    )
                });
        }
    }

    // Group the effects by timepoints.
    for action in actions {
        for effect in action.effects() {
            let t = effect.occurrence().eval(Some(action), env);
            print_info!(env.verbose, "Timepoint {t}");
            print_info!(env.verbose, "Effect {effect}");
            span_actions_map
                .entry(t.clone())
                .and_modify(|a| {
                    a.add_effect(effect.to_span().clone());
                    for p in action.params() {
                        a.add_param(p.clone());
                    }
                })
                .or_insert_with(|| {
                    SpanAction::new(
                        action_name(&t),
                        action_name(&t),
                        action.params().to_vec(),
                        vec![],
                        vec![effect.to_span().clone()],
                    )
                });
        }
    }

    // Calculate epsilon
    env.epsilon = Rational::from(i64::MAX);
    let mut prev_action_and_timepoint: Option<(&SpanAction<E>, &Rational)> = None;
    for (timepoint, action) in span_actions_map.iter() {
        if let Some((prev_action, prev_timepoint)) = prev_action_and_timepoint {
            let diff = timepoint.clone() - prev_timepoint;

            // Check the new calculated epsilon is not too small
            if let Some(min_epsilon) = min_epsilon {
                if diff < *min_epsilon {
                    // The span actions are built in order to match an effect
                    // so the following effects should not be `None`.
                    let prev_effect = prev_action.effects().first();
                    let effect = action.effects().first();
                    debug_assert!(prev_effect.is_some());
                    debug_assert!(effect.is_some());
                    bail!(
                        "Minimal delay of {min_epsilon} between effects ({}) and ({}) is not respected, found {}",
                        prev_effect.unwrap(),
                        effect.unwrap(),
                        env.epsilon
                    );
                }
            }

            // Everything is fine, save it
            if diff < env.epsilon {
                env.epsilon = diff;
            }
        }
        prev_action_and_timepoint = Some((action, timepoint));
    }
    // If present, use the problem's epsilon which is smaller than the calculated one
    if let Some(min_epsilon) = min_epsilon {
        env.epsilon = min_epsilon.clone();
    };

    // Add the conditions start and end timepoints.
    for action in actions {
        for condition in action.conditions() {
            add_condition_terminal(condition, Some(action), env, &mut span_actions_map)?;
        }
    }

    // Add the durative goals start and end timepoints.
    for goal in dur_goals {
        add_condition_terminal(goal, None, env, &mut span_actions_map)?;
    }

    // Add the conditions and durative goals into every timepoints of their interval.
    // Notes: Will be duplicated into start and end timepoints, but it is not a problem.
    for (timepoint, span_action) in span_actions_map.iter_mut() {
        for action in actions {
            for condition in action.conditions() {
                if condition.interval().contains(timepoint, Some(action), env) {
                    span_action.add_condition(condition.to_span().clone());
                    for p in action.params() {
                        span_action.add_param(p.clone());
                    }
                }
            }
        }

        for goal in dur_goals {
            if goal.interval().contains::<E, DurativeAction<E>>(timepoint, None, env) {
                span_action.add_condition(goal.to_span().clone());
            }
        }
    }

    // Extract span actions from the map.
    let span_actions = span_actions_map.values().cloned().collect::<Vec<_>>();

    // Validation.
    let mut extended_actions = span_actions.clone();
    extended_actions.insert(
        0,
        SpanAction::new(EMPTY_ACTION.into(), EMPTY_ACTION.into(), vec![], vec![], vec![]),
    );
    validate_nontemporal(env, &span_actions, span_goals)?
        .into_iter()
        .zip(extended_actions)
        .map(|(s, a)| Ok((timepoint_from_action_name(a.name())?, s)))
        .collect()
}

/* ========================================================================== */
/*                                Hierarchical                                */
/* ========================================================================== */

fn validate_hierarchy<E: Clone + Display + Interpreter>(
    env: &Env<E>,
    actions: &[DurativeAction<E>],
    root_tasks: &HashMap<String, Task<E>>,
    states: &BTreeMap<Rational, State>,
) -> Result<()> {
    /* =========================== Utils Functions ========================== */

    /// Validates the action in the hierarchy:
    /// - the action must be present exactly one time in the decomposition
    /// - the decomposition must contain only actions from the plan
    fn validate_action<E: Clone>(
        env: &Env<E>,
        action: &DurativeAction<E>,
        count_actions: &mut HashMap<String, u8>,
        csp: &mut CspProblem,
    ) -> Result<()> {
        let act_env = action.new_env_with_params(env);

        // Check the decomposition.
        ensure!(
            count_actions.contains_key(action.id()),
            format!(
                "The action with id {} is present in the decomposition but not in the plan",
                action.id()
            )
        );
        count_actions.entry(action.id().to_string()).and_modify(|c| *c += 1);

        // Add the timepoints into the CSP.
        let (start, end) = (
            action.start(&act_env).eval(Some(action), &act_env),
            action.end(&act_env).eval(Some(action), &act_env),
        );
        let (start, end) = (
            CspVariable::new(vec![(start.clone(), start + &env.epsilon)]),
            CspVariable::new(vec![(end.clone(), end + &env.epsilon)]),
        );
        csp.add_variable(CspProblem::start_id(action.id()), start)?;
        csp.add_variable(CspProblem::end_id(action.id()), end)?;
        csp.add_constraint(CspConstraint::Lt(
            CspConstraintTerm::new(CspProblem::start_id(action.id())),
            CspConstraintTerm::new(CspProblem::end_id(action.id())),
        ));
        Ok(())
    }

    /// Validates the method in the hierarchy:
    /// - each subtask must be valid
    /// - the conditions and the constraints are valid
    fn validate_method<E: Clone + Display + Interpreter>(
        env: &Env<E>,
        method: &Method<E>,
        count_actions: &mut HashMap<String, u8>,
        states: &BTreeMap<Rational, State>,
        csp: &mut CspProblem,
        empty_methods: &mut Vec<String>,
    ) -> Result<()> {
        let mut meth_env = method.new_env_with_params(env);
        meth_env.set_method(method.clone());
        let start_id = CspProblem::start_id(method.id());
        let end_id = CspProblem::end_id(method.id());

        // Check each subtask and constraint the method to be minimal and to contain its subtasks.
        let mut start_constraints: Vec<CspConstraint> = vec![];
        let mut end_constraints: Vec<CspConstraint> = vec![];
        for (_, subtask) in method.subtasks().iter() {
            // Check the subtask.
            match subtask {
                models::method::Subtask::Action(a) => validate_action(&meth_env, a, count_actions, csp)?,
                models::method::Subtask::Task(t) => {
                    validate_task(&meth_env, t, count_actions, states, csp, empty_methods)?
                }
            };

            // Constraint the method.
            start_constraints.push(CspConstraint::Equals(
                CspConstraintTerm::new(start_id.clone()),
                CspConstraintTerm::new(CspProblem::start_id(subtask.id())),
            ));
            end_constraints.push(CspConstraint::Equals(
                CspConstraintTerm::new(end_id.clone()),
                CspConstraintTerm::new(CspProblem::end_id(subtask.id())),
            ));
        }
        if !start_constraints.is_empty() {
            csp.add_constraint(CspConstraint::Or(start_constraints));
            csp.add_constraint(CspConstraint::Or(end_constraints));
        }

        // Search the states where the method is applicable.
        let mut domain: Vec<(Rational, Rational)> = Vec::new();
        let mut lb = None;
        for (timepoint, state) in states.iter() {
            let mut new_env = meth_env.clone();
            new_env.set_state(state.clone());

            for condition in method.conditions().iter() {
                if condition.interval().contains(timepoint, Some(method), &new_env) || method.subtasks().is_empty() {
                    if condition.to_span().is_valid(&new_env)? {
                        if lb.is_none() {
                            lb = Some(timepoint);
                        }
                    } else if let Some(l) = lb {
                        domain.push((l.clone(), (timepoint - &env.epsilon).clone()));
                        lb = None;
                    }
                }
            }
        }
        if let Some(l) = lb {
            domain.push((l.clone(), Rational::from(INT_CST_MAX)));
        }

        // Create CSP variables matching the states.
        let start = CspVariable::new(domain.to_vec());
        let end = CspVariable::new(domain.to_vec());
        csp.add_variable(start_id.clone(), start)?;
        csp.add_variable(end_id.clone(), end)?;
        if method.subtasks().is_empty() {
            // We consider that an empty method is instantaneous.
            csp.add_constraint(CspConstraint::Equals(
                CspConstraintTerm::new(start_id),
                CspConstraintTerm::new(end_id),
            ));
            empty_methods.push(method.id().to_string());
        } else {
            csp.add_constraint(CspConstraint::Lt(
                CspConstraintTerm::new(start_id),
                CspConstraintTerm::new(end_id),
            ));
        }

        // Add the constraints between the subtasks.
        for constraint in method.constraints().iter() {
            csp.add_constraint(constraint.convert_to_csp_constraint(&meth_env)?);
        }

        Ok(())
    }

    /// Validates the task in the hierarchy:
    /// - the refiner must be valid
    fn validate_task<E: Clone + Display + Interpreter>(
        env: &Env<E>,
        task: &Task<E>,
        count_actions: &mut HashMap<String, u8>,
        states: &BTreeMap<Rational, State>,
        csp: &mut CspProblem,
        empty_methods: &mut Vec<String>,
    ) -> Result<()> {
        let task_env = task.new_env_with_params(env);
        match task.refiner() {
            models::task::Refiner::Method(m) => {
                validate_method(&task_env, m, count_actions, states, csp, empty_methods)
            }
            models::task::Refiner::Action(a) => validate_action(&task_env, a, count_actions, csp),
        }
    }

    /* ============================ Function Body =========================== */

    // Count the actions to check that each action of the plan is present exactly one time in the decomposition.
    let mut count_actions: HashMap<String, u8> = actions.iter().map(|a| (a.id().to_string(), 0u8)).collect();

    // Regroups the methods without subtasks in order to adapt the constraints at the end.
    let mut empty_methods: Vec<String> = vec![];

    // A CSP problem to check the constraints between the different tasks.
    let mut csp = CspProblem::default();
    // TODO (Roland) - Initialise it with the constraints of the initial task network.

    // Check each root task.
    for (_, task) in root_tasks.iter() {
        validate_task(env, task, &mut count_actions, states, &mut csp, &mut empty_methods)?;
    }

    // Validate the count of the actions.
    for (action_id, count) in count_actions.iter() {
        match count.cmp(&1) {
            std::cmp::Ordering::Less => {
                bail!("The action with id {action_id} is present in the plan but not in the decomposition")
            }
            std::cmp::Ordering::Equal => {} // Everything is OK
            std::cmp::Ordering::Greater => {
                bail!("The action with id {action_id} is present more than one time in the decomposition")
            }
        };
    }

    // Adapt the CSP constraint for the empty methods, i.e. without subtasks.
    // Empty methods are present in our simulation with an instantaneous execution time in order to check they are applicable.
    // However, they are usually not present in generated plans.
    // Therefore, ordering constraints `<` need to be relaxed as `<=` for these methods.
    csp.map_constraints(|constraint| match constraint {
        CspConstraint::Lt(lhs, rhs) => {
            let mut found = false;
            for meth in empty_methods.iter() {
                if CspProblem::start_id(meth) == rhs.id().to_string()
                    || CspProblem::end_id(meth) == lhs.id().to_string()
                {
                    found = true;
                    break;
                }
            }

            if found {
                CspConstraint::Le(lhs.clone(), rhs.clone())
            } else {
                constraint.clone()
            }
        }
        _ => constraint.clone(),
    });

    // Validate the CSP problem.
    ensure!(csp.is_valid(), "The constraints between the tasks are not verified");
    Ok(())
}
