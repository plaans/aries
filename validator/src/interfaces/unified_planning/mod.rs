use crate::interfaces::unified_planning::factories::expression::int as cint;
use crate::{
    interfaces::unified_planning::utils::rational,
    models::{
        action::{Action, DurativeAction, SpanAction},
        condition::{Condition, DurativeCondition, SpanCondition},
        csp::{CspProblem, CspVariable},
        effects::{DurativeEffect, EffectKind as EffectKindModel, SpanEffect},
        env::Env,
        method::{Method, Subtask},
        parameter::Parameter,
        task::{Refiner, Task},
        time::{TemporalInterval, TemporalIntervalExpression, Timepoint},
        value::Value,
    },
    print_info, print_warn, procedures,
    traits::interpreter::Interpreter,
    validate,
};
use anyhow::{bail, ensure, Context, Result};
use malachite::Rational;
use std::{collections::HashMap, convert::TryInto, ops::Deref};
use unified_planning::Parameter as upParam;
use unified_planning::{
    effect_expression::EffectKind, ActionInstance, Activity, Atom, Expression, Feature, Goal, Hierarchy, Plan,
    PlanHierarchy, Problem, TimedEffect,
};

use self::{constants::*, utils::state_variable_to_signature};

mod constants;
mod expression;
mod factories;
mod time;
mod utils;

/* ========================================================================== */
/*                               Entry Function                               */
/* ========================================================================== */

/// Validates the plan for the given UPF problem.
pub fn validate_upf(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    print_info!(verbose, "Start the validation");
    if is_schedule(problem, plan)? {
        validate_schedule(problem, plan, verbose)
    } else {
        validate_non_schedule(problem, plan, verbose)
    }
}

fn validate_schedule(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    // Check schedule problem and plan format:
    //   - Non temporal
    //   - No hierarchy
    //   - No actions (activities instead)
    //   - No goals
    debug_assert!(is_schedule(problem, plan)?);
    ensure!(is_temporal(problem));
    ensure!(problem.hierarchy.is_none());
    ensure!(plan.hierarchy.is_none());
    ensure!(problem.actions.is_empty());
    ensure!(plan.actions.is_empty());
    ensure!(problem.goals.is_empty());
    validate(
        &mut build_env(problem, plan, verbose)?,
        &build_activities(problem, plan, verbose)?,
        None,
        &build_goals(problem, verbose, true)?,
        &build_timed_effect(problem, verbose)?,
        true,
        &epsilon_from_problem(problem),
    )?;
    validate_schedule_constraints(problem, plan, verbose)
}

fn validate_schedule_constraints(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    let mut csp = CspProblem::default();
    let mut env = build_env(problem, plan, verbose)?;
    env.epsilon = if let Some(e) = problem.epsilon.as_ref() {
        Rational::from_signeds(e.numerator, e.denominator)
    } else {
        1.into()
    };

    // Create CSP Variables.
    let var_assign = &plan.schedule.as_ref().unwrap().variable_assignments;
    for activity in plan.schedule.as_ref().unwrap().activities.iter() {
        // Names
        let start_id = CspProblem::start_id(activity);
        let end_id = CspProblem::end_id(activity);
        // Assignment values
        let start_value = var_assign
            .get(&start_id)
            .context(format!("No value assignment for the variable {start_id}"))?;
        let end_value = var_assign
            .get(&end_id)
            .context(format!("No value assignment for the variable {end_id}"))?;
        // CSP Variables
        let start_var = CspVariable::new(vec![(rational(start_value)?, rational(start_value)? + &env.epsilon)]);
        let end_var = CspVariable::new(vec![(rational(end_value)?, rational(end_value)? + &env.epsilon)]);
        // Add them
        csp.add_variable(start_id, start_var)?;
        csp.add_variable(end_id, end_var)?;
    }

    // Create CSP Constraints.
    for constr in problem.scheduling_extension.as_ref().unwrap().constraints.iter() {
        csp.add_constraint(constr.convert_to_csp_constraint(&env)?);
    }

    // Validate the CSP.
    ensure!(
        csp.is_valid(),
        "The constraints between the activities are not verified"
    );
    Ok(())
}

fn validate_non_schedule(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    debug_assert!(!is_schedule(problem, plan)?);
    let temporal = is_temporal(problem);
    let actions = build_actions(problem, plan, verbose, temporal)?;
    validate(
        &mut build_env(problem, plan, verbose)?,
        &actions,
        build_root_tasks(problem, plan, &Action::into_durative(&actions), verbose)?.as_ref(),
        &build_goals(problem, verbose, temporal)?,
        &build_timed_effect(problem, verbose)?,
        temporal,
        &epsilon_from_problem(problem),
    )
}

/* ========================================================================== */
/*                             Environment Factory                            */
/* ========================================================================== */

/// Builds the initial environment from the problem.
fn build_env(problem: &Problem, plan: &Plan, verbose: bool) -> Result<Env<Expression>> {
    print_info!(verbose, "Creation of the initial state");
    let mut env = Env::default();
    env.verbose = verbose;
    env.discrete_time = is_discrete_time(problem);
    env.schedule_problem = is_schedule(problem, plan)?;
    ensure!(!env.schedule_problem || env.discrete_time);

    // Bounds types.
    for t in problem.types.iter() {
        env.bound_type(t.type_name.clone(), t.parent_type.clone());
    }

    // Bounds objects.
    for o in problem.objects.iter() {
        env.bound(o.r#type.clone(), o.name.clone(), o.name.clone().into());
    }

    // Bounds fluents to default values.
    for fluent in problem.fluents.iter() {
        if fluent.default_value.is_none() {
            print_warn!(verbose, "Fluent {} has no default value", fluent.name);
            continue;
        }

        let fluent_name = format!("{} -- {}", fluent.name, fluent.value_type);
        let default = fluent.default_value.as_ref().unwrap().eval(&env)?;
        let combinations = generate_fluent_parameters_combinations(&env, &fluent.parameters)?;

        for combination in combinations {
            let mut signature = vec![Value::from(fluent_name.clone())];
            signature.extend(combination);
            env.bound_fluent(signature.clone(), default.clone())?;
        }
    }

    // Bounds fluents of the initial state.
    for assignment in problem.initial_state.iter() {
        let k = state_variable_to_signature(&env, assignment.fluent.as_ref().context("Assignment without fluent")?)?;
        let v = assignment
            .value
            .as_ref()
            .context("Assignment without value")?
            .eval(&env)?;
        env.bound_fluent(k, v)?;
    }

    // Bounds procedures.
    env.bound_procedure(UP_AND.into(), procedures::and);
    env.bound_procedure(UP_OR.into(), procedures::or);
    env.bound_procedure(UP_NOT.into(), procedures::not);
    env.bound_procedure(UP_IMPLIES.into(), procedures::implies);
    env.bound_procedure(UP_EQUALS.into(), procedures::equals);
    env.bound_procedure(UP_LT.into(), procedures::lt);
    env.bound_procedure(UP_LE.into(), procedures::le);
    env.bound_procedure(UP_PLUS.into(), procedures::plus);
    env.bound_procedure(UP_MINUS.into(), procedures::minus);
    env.bound_procedure(UP_TIMES.into(), procedures::times);
    env.bound_procedure(UP_DIV.into(), procedures::div);
    env.bound_procedure(UP_EXISTS.into(), procedures::exists);
    env.bound_procedure(UP_FORALL.into(), procedures::forall);
    env.bound_procedure(UP_IFF.into(), procedures::iff);
    env.bound_procedure(UP_END.into(), procedures::end);
    env.bound_procedure(UP_START.into(), procedures::start);

    // Returns the environment.
    Ok(env)
}

fn generate_fluent_parameters_combinations(
    env: &Env<Expression>,
    parameters: &Vec<upParam>,
) -> Result<Vec<Vec<Value>>> {
    if parameters.is_empty() {
        return Ok(vec![vec![]]);
    }

    let mut combinations = Vec::new();
    let first_param = &parameters[0];
    let next_params = &parameters[1..];
    let objects = env
        .get_objects(&first_param.r#type.to_string())
        .context(format!("No objects of type {}", first_param.r#type))?;

    for obj in objects {
        let next_combinations = generate_fluent_parameters_combinations(env, &next_params.to_vec())?;
        for next_parameters in next_combinations {
            let mut combination = vec![obj.clone()];
            combination.extend(next_parameters);
            combinations.push(combination);
        }
    }

    Ok(combinations)
}

/* ========================================================================== */
/*                               Actions Factory                              */
/* ========================================================================== */

/// Builds the actions from the problem and the plan.
fn build_actions(problem: &Problem, plan: &Plan, verbose: bool, temporal: bool) -> Result<Vec<Action<Expression>>> {
    /* =========================== Utils Functions ========================== */

    /// Creates the span or durative action.
    fn build_action(
        problem: &Problem,
        a: &ActionInstance,
        temporal: bool,
        default_id: String,
    ) -> Result<Action<Expression>> {
        let pb_a = &get_pb_action(problem, a)?;
        let id = if a.id.is_empty() { default_id } else { a.id.clone() };

        Ok(if temporal {
            let start = a
                .start_time
                .as_ref()
                .context("No start timepoint for a temporal action")?;
            let start = Rational::from_signeds(start.numerator, start.denominator);
            let end = a.end_time.as_ref().context("No end timepoint for a temporal action")?;
            let end = Rational::from_signeds(end.numerator, end.denominator);
            let duration = if let Some(dur) = &pb_a.duration {
                dur.controllable_in_bounds
                    .as_ref()
                    .context("Duration without interval")?
                    .clone()
                    .try_into()?
            } else {
                TemporalIntervalExpression::new(cint(0), cint(0), false, false)
            };

            Action::Durative(DurativeAction::new(
                a.action_name.clone(),
                id,
                build_params(pb_a, a)?,
                build_conditions_durative(pb_a)?,
                build_effects_durative(pb_a)?,
                Timepoint::fixed(start),
                Timepoint::fixed(end),
                Some(duration),
            ))
        } else {
            Action::Span(SpanAction::new(
                a.action_name.clone(),
                id,
                build_params(pb_a, a)?,
                build_conditions_span(pb_a)?,
                build_effects_span(pb_a)?,
            ))
        })
    }

    /// Builds the conditions for a span action.
    fn build_conditions_span(a: &unified_planning::Action) -> Result<Vec<SpanCondition<Expression>>> {
        a.conditions
            .iter()
            .map(|c| {
                Ok(SpanCondition::new(
                    c.cond.as_ref().context("Condition without expression")?.clone(),
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the conditions for a durative action.
    fn build_conditions_durative(a: &unified_planning::Action) -> Result<Vec<DurativeCondition<Expression>>> {
        let cond = build_conditions_span(a)?;
        a.conditions
            .iter()
            .zip(cond)
            .map(|(c, s)| {
                let interval = if let Some(span) = &c.span {
                    span.clone().try_into()?
                } else {
                    TemporalInterval::overall()
                };

                Ok(DurativeCondition::from_span(s, interval))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the effects for a span action.
    fn build_effects_span(a: &unified_planning::Action) -> Result<Vec<SpanEffect<Expression>>> {
        a.effects
            .iter()
            .map(|e| {
                let expr = e.effect.as_ref().context("Effect without expression")?;
                Ok(SpanEffect::new(
                    expr.clone().fluent.context("Effect without fluent")?.list,
                    expr.clone().value.context("Effect without value")?,
                    match expr.clone().kind() {
                        EffectKind::Assign => EffectKindModel::Assign,
                        EffectKind::Increase => EffectKindModel::Increase,
                        EffectKind::Decrease => EffectKindModel::Decrease,
                    },
                    if let Some(cond) = expr.clone().condition {
                        vec![SpanCondition::new(cond)]
                    } else {
                        vec![]
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the effects for a durative action.
    fn build_effects_durative(a: &unified_planning::Action) -> Result<Vec<DurativeEffect<Expression>>> {
        let eff = build_effects_span(a)?;
        a.effects
            .iter()
            .zip(eff)
            .map(|(e, s)| {
                let occurrence = if let Some(time) = &e.occurrence_time {
                    time.clone().try_into()?
                } else {
                    Timepoint::at_end()
                };

                Ok(DurativeEffect::from_span(s, occurrence))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Creates the environment to map the Action to its Instance.
    fn build_params(pb_a: &unified_planning::Action, a: &ActionInstance) -> Result<Vec<Parameter>> {
        ensure!(pb_a.parameters.len() == a.parameters.len());
        pb_a.parameters
            .iter()
            .zip(a.parameters.iter())
            .map(|(pb_p, p)| {
                Ok(Parameter::new(
                    pb_p.name.clone(),
                    pb_p.r#type.clone(),
                    p.content.as_ref().context("Atom without content")?.clone().into(),
                ))
            })
            .collect::<Result<Vec<Parameter>>>()
    }

    /// Finds the action in the problem based on its name.
    fn get_pb_action(p: &Problem, a: &ActionInstance) -> Result<unified_planning::Action> {
        p.actions
            .iter()
            .find(|&x| x.name == a.action_name)
            .context(format!("No action named {:?} in the problem", a.action_name))
            .cloned()
    }

    /* ============================ Function Body =========================== */

    print_info!(verbose, "Creation of the actions");
    ensure!(!is_schedule(problem, plan)?);
    plan.actions
        .iter()
        .enumerate()
        .map(|(i, a)| build_action(problem, a, temporal, i.to_string()))
        .collect::<Result<Vec<_>>>()
}

/// Builds the actions from the activities of the problem.
fn build_activities(problem: &Problem, plan: &Plan, verbose: bool) -> Result<Vec<Action<Expression>>> {
    /* =========================== Utils Functions ========================== */
    /// Builds the Action corresponding to the given Activity.
    fn build_activity(a: &Activity, var_assign: &HashMap<String, Atom>) -> Result<Action<Expression>> {
        let start = rational(
            var_assign
                .get(&format!("{}.start", a.name))
                .context(format!("No start timepoint for activity {}", a.name))?,
        )?;
        let end = rational(
            var_assign
                .get(&format!("{}.end", a.name))
                .context(format!("No end timepoint for activity {}", a.name))?,
        )?;

        Ok(Action::Durative(DurativeAction::new(
            a.name.clone(),
            a.name.clone(),
            build_parameters(a, var_assign)?,
            build_conditions(a)?,
            build_effects(a)?,
            Timepoint::fixed(start),
            Timepoint::fixed(end),
            Some(
                a.duration
                    .as_ref()
                    .context("Durative action without duration")?
                    .controllable_in_bounds
                    .as_ref()
                    .context("Duration without interval")?
                    .clone()
                    .try_into()?,
            ),
        )))
    }

    /// Builds the conditions for an activity.
    fn build_conditions(a: &Activity) -> Result<Vec<DurativeCondition<Expression>>> {
        a.conditions
            .iter()
            .map(|c| {
                Ok(DurativeCondition::new(
                    c.cond.as_ref().context("Condition without expression")?.clone(),
                    c.span
                        .clone()
                        .context("Durative condition without temporal interval")?
                        .try_into()?,
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the effects for an activity.
    fn build_effects(a: &Activity) -> Result<Vec<DurativeEffect<Expression>>> {
        a.effects
            .iter()
            .map(|e| {
                let expr = e.effect.as_ref().context("Effect without expression")?;
                Ok(DurativeEffect::new(
                    expr.clone().fluent.context("Effect without fluent")?.list,
                    expr.clone().value.context("Effect without value")?,
                    match expr.kind() {
                        EffectKind::Assign => EffectKindModel::Assign,
                        EffectKind::Increase => EffectKindModel::Increase,
                        EffectKind::Decrease => EffectKindModel::Decrease,
                    },
                    if let Some(cond) = expr.clone().condition {
                        vec![SpanCondition::new(cond)]
                    } else {
                        vec![]
                    },
                    e.occurrence_time
                        .clone()
                        .context("Durative effect without occurrence time")?
                        .try_into()?,
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the parameters for an activity.
    fn build_parameters(a: &Activity, var_assign: &HashMap<String, Atom>) -> Result<Vec<Parameter>> {
        a.parameters
            .iter()
            .map(|p| {
                let value: Value = var_assign
                    .get(&p.name)
                    .context(format!("Cannot find the value of the parameter {}", p.name))?
                    .content
                    .as_ref()
                    .context("Atom without content")?
                    .clone()
                    .into();

                Ok(Parameter::new(p.name.clone(), p.r#type.clone(), value))
            })
            .collect::<Result<_>>()
    }

    /* ============================ Function Body =========================== */

    print_info!(verbose, "Creation of the activities");
    ensure!(is_schedule(problem, plan)?);
    let schedule = problem
        .scheduling_extension
        .as_ref()
        .context("Schedule problem without schedule extension")?;
    let var_assign = &plan
        .schedule
        .as_ref()
        .context("Schedule plan without schedule")?
        .variable_assignments;

    // TODO (Roland) - Support schedule problems with variables in the validator
    ensure!(
        schedule.variables.is_empty(),
        "Unsupported schedule problems with variables"
    );

    schedule
        .activities
        .iter()
        .map(|a| build_activity(a, var_assign))
        .collect::<Result<Vec<_>>>()
}

/* ========================================================================== */
/*                                Goals Factory                               */
/* ========================================================================== */

/// Builds the goals from the problem.
fn build_goals(problem: &Problem, verbose: bool, temporal: bool) -> Result<Vec<Condition<Expression>>> {
    /* =========================== Utils Functions ========================== */

    /// Creates the span or durative goal.
    fn build_goal(g: &Goal, temporal: bool) -> Result<Condition<Expression>> {
        let expr = g.goal.as_ref().context("Goal without expression")?.clone();

        Ok(if temporal {
            if let Some(timing) = g.timing.clone() {
                Condition::Durative(DurativeCondition::new(expr, timing.try_into()?))
            } else {
                Condition::Span(SpanCondition::new(expr))
            }
        } else {
            Condition::Span(SpanCondition::new(expr))
        })
    }

    /* ============================ Function Body =========================== */

    print_info!(verbose, "Creation of the goals");
    problem
        .goals
        .iter()
        .map(|g| build_goal(g, temporal))
        .collect::<Result<Vec<_>>>()
}

/* ========================================================================== */
/*                              Root Task Factory                             */
/* ========================================================================== */

/// Builds the root tasks of the hierarchy.
fn build_root_tasks(
    problem: &Problem,
    plan: &Plan,
    actions: &[DurativeAction<Expression>],
    verbose: bool,
) -> Result<Option<HashMap<String, Task<Expression>>>> {
    /* =========================== Utils Functions ========================== */

    fn build_subtask(
        pb_hierarchy: &Hierarchy,
        plan_hierarchy: &PlanHierarchy,
        actions: &[DurativeAction<Expression>],
        subtasks: &[unified_planning::Task],
        task_id: &String,
        refiner_id: &String,
    ) -> Result<Subtask<Expression>> {
        let task = subtasks.iter().find(|t| t.id == *task_id).context(format!(
            "Cannot find a task with the id {task_id} in the given subtasks {subtasks:?}"
        ))?;

        Ok(
            if let Some(pb_task) = pb_hierarchy.abstract_tasks.iter().find(|t| t.name == task.task_name) {
                // There is task with the searched named.
                Subtask::Task(build_task(
                    pb_hierarchy,
                    plan_hierarchy,
                    actions,
                    refiner_id,
                    task,
                    pb_task,
                )?)
            } else {
                // Try to find the matching action.
                Subtask::Action(
                    actions
                        .iter()
                        .find(|a| a.id() == refiner_id)
                        .context(format!(
                            "Cannot find a task or an action with the name {}",
                            task.task_name
                        ))?
                        .clone(),
                )
            },
        )
    }

    fn build_task(
        pb_hierarchy: &Hierarchy,
        plan_hierarchy: &PlanHierarchy,
        actions: &[DurativeAction<Expression>],
        refiner_id: &String,
        task: &unified_planning::Task,
        pb_task: &unified_planning::AbstractTaskDeclaration,
    ) -> Result<Task<Expression>> {
        let refiner = if let Some(action) = actions.iter().find(|a| a.id() == refiner_id) {
            Refiner::Action(action.clone())
        } else {
            Refiner::Method(build_method(pb_hierarchy, plan_hierarchy, actions, refiner_id)?)
        };

        ensure!(&pb_task.parameters.len() == &task.parameters.len());
        let params = pb_task
            .parameters
            .iter()
            .zip(task.parameters.iter())
            .map(|(pb_p, p)| {
                Ok(Parameter::new(
                    pb_p.name.clone(),
                    pb_p.r#type.clone(),
                    p.atom
                        .as_ref()
                        .context("Only atoms are supported as parameters for a task")?
                        .content
                        .as_ref()
                        .context("Atom without content")?
                        .clone()
                        .into(),
                ))
            })
            .collect::<Result<Vec<Parameter>>>()?;

        Ok(Task::new(
            task.task_name.to_string(),
            task.id.to_string(),
            params,
            refiner,
        ))
    }

    fn build_method(
        pb_hierarchy: &Hierarchy,
        plan_hierarchy: &PlanHierarchy,
        actions: &[DurativeAction<Expression>],
        id: &String,
    ) -> Result<Method<Expression>> {
        let plan_meth = plan_hierarchy
            .methods
            .iter()
            .find(|m| m.id == *id)
            .context(format!("Cannot find a method with the id {id}"))?;

        let pb_meth = pb_hierarchy
            .methods
            .iter()
            .find(|m| m.name == plan_meth.method_name)
            .context(format!("Cannot find a method with the name {}", plan_meth.method_name))?;

        ensure!(plan_meth.parameters.len() == pb_meth.parameters.len());
        let params = pb_meth
            .parameters
            .iter()
            .zip(plan_meth.parameters.iter())
            .map(|(pb_p, p)| {
                Ok(Parameter::new(
                    pb_p.name.clone(),
                    pb_p.r#type.clone(),
                    p.content.as_ref().context("Atom without content")?.clone().into(),
                ))
            })
            .collect::<Result<_>>()?;

        let conditions = pb_meth
            .conditions
            .iter()
            .map(|c| {
                let span = SpanCondition::new(c.cond.as_ref().context("Condition without expression")?.clone());
                let interval = if let Some(interval) = &c.span {
                    interval.clone().try_into()?
                } else {
                    TemporalInterval::at_start()
                };
                Ok(DurativeCondition::from_span(span, interval))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Method::new(
            pb_meth.name.to_string(),
            id.to_string(),
            params,
            conditions,
            pb_meth.constraints.to_vec(),
            build_subtasks_from_hashmap(
                &plan_meth.subtasks,
                pb_hierarchy,
                plan_hierarchy,
                actions,
                pb_meth.subtasks.deref(),
            )?,
        ))
    }

    fn build_subtasks_from_hashmap(
        map: &HashMap<String, String>,
        pb_hierarchy: &Hierarchy,
        plan_hierarchy: &PlanHierarchy,
        actions: &[DurativeAction<Expression>],
        subtasks: &[unified_planning::Task],
    ) -> Result<HashMap<String, Subtask<Expression>>> {
        map.iter()
            .map(|(task_id, refiner_id)| {
                Ok((
                    task_id.to_string(),
                    build_subtask(pb_hierarchy, plan_hierarchy, actions, subtasks, task_id, refiner_id)?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()
    }

    /* ============================ Function Body =========================== */

    print_info!(verbose, "Creation of the root task");

    Ok(if let Some(plan_hierarchy) = &plan.hierarchy {
        let pb_hierarchy = problem
            .hierarchy
            .as_ref()
            .context("The plan is hierarchical but the problem is not")?;

        let task_network = pb_hierarchy
            .initial_task_network
            .as_ref()
            .context("No initial task network in the problem hierarchy")?;

        Some(
            build_subtasks_from_hashmap(
                &plan_hierarchy.root_tasks,
                pb_hierarchy,
                plan_hierarchy,
                actions,
                task_network.subtasks.deref(),
            )?
            .into_iter()
            .map(|(id, st)| match st {
                Subtask::Action(_) => bail!("Found an action in the root tasks"),
                Subtask::Task(t) => Ok((id, t)),
            })
            .collect::<Result<_>>()?,
        )
    } else {
        None
    })
}

/* ========================================================================== */
/*                            Timed Effects Factory                           */
/* ========================================================================== */

/// Builds the timed effects of the problem.
fn build_timed_effect(problem: &Problem, verbose: bool) -> Result<Vec<DurativeEffect<Expression>>> {
    /* =========================== Utils Functions ========================== */

    fn build_timed_effect_action(te: &TimedEffect) -> Result<DurativeEffect<Expression>> {
        let expr = te.effect.as_ref().context("Timed effect without expression")?;
        Ok(DurativeEffect::new(
            expr.clone().fluent.context("Effect without fluent")?.list,
            expr.clone().value.context("Effect without value")?,
            match expr.kind() {
                EffectKind::Assign => EffectKindModel::Assign,
                EffectKind::Increase => EffectKindModel::Increase,
                EffectKind::Decrease => EffectKindModel::Decrease,
            },
            if let Some(cond) = expr.clone().condition {
                vec![SpanCondition::new(cond)]
            } else {
                vec![]
            },
            te.occurrence_time
                .clone()
                .context("Timed effect without occurrence time")?
                .try_into()?,
        ))
    }

    /* ============================ Function Body =========================== */

    print_info!(verbose, "Creation of the timed effects");
    problem
        .timed_effects
        .iter()
        .map(build_timed_effect_action)
        .collect::<Result<Vec<_>>>()
}

/* ========================================================================== */
/*                          Problem and Plan Features                         */
/* ========================================================================== */

/// Returns the optional epsilon stored in the problem.
fn epsilon_from_problem(problem: &Problem) -> Option<Rational> {
    problem
        .epsilon
        .as_ref()
        .map(|e| Rational::from_signeds(e.numerator, e.denominator))
}

/// Returns whether the problem and the plan are schedule.
///
/// Returns an error if the problem and the plan does not match.
fn is_schedule(problem: &Problem, plan: &Plan) -> Result<bool> {
    let pb = problem.scheduling_extension.is_some();
    let pl = plan.schedule.is_some();
    ensure!(pb == pl);
    Ok(pb)
}

/// Returns whether the problem has continuous time feature.
fn is_continuous_time(problem: &Problem) -> bool {
    problem.features.contains(&Feature::ContinuousTime.into())
}

/// Returns whether the problem has discrete time feature.
fn is_discrete_time(problem: &Problem) -> bool {
    problem.features.contains(&Feature::DiscreteTime.into())
}

/// Returns whether the problem is temporal.
fn is_temporal(problem: &Problem) -> bool {
    is_continuous_time(problem) || is_discrete_time(problem)
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use anyhow::bail;

    use crate::{
        interfaces::unified_planning::factories::{expression, plan, problem},
        models::{
            parameter::Parameter,
            time::{TemporalInterval, TemporalIntervalExpression},
        },
    };

    use super::*;

    #[test]
    fn test_build_env() -> Result<()> {
        // Non temporal problem
        let mut p = problem::mock_nontemporal();
        let pl = plan::mock_nontemporal();
        let mut e = Env::<Expression>::default();
        assert!(!e.verbose);
        assert!(!e.discrete_time);
        assert!(!e.schedule_problem);

        // Types
        e.bound_type("locatable".into(), "".into());
        e.bound_type("robot".into(), "locatable".into());
        e.bound_type("location".into(), "locatable".into());

        // Objects
        e.bound("robot".into(), "R1".into(), "R1".into());
        e.bound("location".into(), "L1".into(), "L1".into());
        e.bound("location".into(), "L2".into(), "L2".into());

        // Fluents
        e.bound_fluent(vec!["loc -- location".into(), "R1".into()], "L1".into())?;

        // Procedures
        e.bound_procedure(UP_AND.into(), procedures::and);
        e.bound_procedure(UP_OR.into(), procedures::or);
        e.bound_procedure(UP_NOT.into(), procedures::not);
        e.bound_procedure(UP_IMPLIES.into(), procedures::implies);
        e.bound_procedure(UP_EQUALS.into(), procedures::equals);
        e.bound_procedure(UP_LT.into(), procedures::lt);
        e.bound_procedure(UP_LE.into(), procedures::le);
        e.bound_procedure(UP_PLUS.into(), procedures::plus);
        e.bound_procedure(UP_MINUS.into(), procedures::minus);
        e.bound_procedure(UP_TIMES.into(), procedures::times);
        e.bound_procedure(UP_DIV.into(), procedures::div);
        e.bound_procedure(UP_EXISTS.into(), procedures::exists);
        e.bound_procedure(UP_FORALL.into(), procedures::forall);
        e.bound_procedure(UP_IFF.into(), procedures::iff);
        e.bound_procedure(UP_END.into(), procedures::end);
        e.bound_procedure(UP_START.into(), procedures::start);

        assert_eq!(e, build_env(&p, &pl, false)?, "Non temporal problem");

        // Continuous temporal problem
        let pl = plan::mock_temporal();
        p.features.push(Feature::ContinuousTime.into());
        e.discrete_time = false;
        assert_eq!(e, build_env(&p, &pl, false)?, "Continuous temporal problem");

        // Discrete temporal problem
        p.features.push(Feature::DiscreteTime.into());
        e.discrete_time = true;
        assert_eq!(e, build_env(&p, &pl, false)?, "Discrete temporal problem");

        // Schedule problem
        let p = problem::mock_schedule();
        let pl = plan::mock_schedule();
        assert!(build_env(&p, &pl, false)?.schedule_problem);

        Ok(())
    }

    #[test]
    fn test_build_actions_nontemporal() -> Result<()> {
        let problem = problem::mock_nontemporal();
        let plan = plan::mock_nontemporal();

        let robot_type = "robot";
        let robot_param = "r";
        let r1 = "R1";
        let loc_type = "location";
        let loc_fluent = "loc";
        let loc1 = "L1";
        let loc2 = "L2";
        let move_action = "move";

        let loc_robot = expression::state_variable(vec![
            expression::fluent_symbol(loc_fluent),
            expression::parameter(robot_param, robot_type),
        ]);

        let expected = vec![Action::Span(SpanAction::new(
            move_action.into(),
            "a1".into(),
            vec![
                Parameter::new(robot_param.into(), robot_type.into(), r1.into()),
                Parameter::new("from".into(), loc_type.into(), loc1.into()),
                Parameter::new("to".into(), loc_type.into(), loc2.into()),
            ],
            vec![SpanCondition::new(expression::function_application(vec![
                expression::function_symbol(UP_EQUALS),
                loc_robot.clone(),
                expression::parameter("from", loc_type),
            ]))],
            vec![SpanEffect::new(
                loc_robot.list,
                expression::parameter("to", loc_type),
                EffectKindModel::Assign,
                vec![],
            )],
        ))];

        assert_eq!(build_actions(&problem, &plan, false, false)?, expected);
        Ok(())
    }

    #[test]
    fn test_build_actions_temporal() -> Result<()> {
        let problem = problem::mock_temporal();
        let plan = plan::mock_temporal();

        let robot_type = "robot";
        let robot_param = "r";
        let r1 = "R1";
        let loc_type = "location";
        let loc_fluent = "loc";
        let loc1 = "L1";
        let loc2 = "L2";
        let loc_u = "Lu";
        let move_action = "move";

        let loc_robot = expression::state_variable(vec![
            expression::fluent_symbol(loc_fluent),
            expression::parameter(robot_param, robot_type),
        ]);

        let expected = vec![Action::Durative(DurativeAction::new(
            move_action.into(),
            "a1".into(),
            vec![
                Parameter::new(robot_param.into(), robot_type.into(), r1.into()),
                Parameter::new("from".into(), loc_type.into(), loc1.into()),
                Parameter::new("to".into(), loc_type.into(), loc2.into()),
            ],
            vec![DurativeCondition::new(
                expression::function_application(vec![
                    expression::function_symbol(UP_EQUALS),
                    loc_robot.clone(),
                    expression::parameter("from", loc_type),
                ]),
                TemporalInterval::at_start(),
            )],
            vec![
                DurativeEffect::new(
                    loc_robot.list.clone(),
                    expression::parameter(loc_u, loc_type),
                    EffectKindModel::Assign,
                    vec![],
                    Timepoint::at_start(),
                ),
                DurativeEffect::new(
                    loc_robot.list,
                    expression::parameter("to", loc_type),
                    EffectKindModel::Assign,
                    vec![],
                    Timepoint::at_end(),
                ),
            ],
            Timepoint::fixed(0.into()),
            Timepoint::fixed(5.into()),
            Some(TemporalIntervalExpression::new(
                expression::int(5),
                expression::int(5),
                false,
                false,
            )),
        ))];

        assert_eq!(build_actions(&problem, &plan, false, true)?, expected);
        Ok(())
    }

    #[test]
    fn build_actions_schedule() {
        let problem = problem::mock_schedule();
        let plan = plan::mock_schedule();
        assert!(build_actions(&problem, &plan, false, false).is_err());
        assert!(build_actions(&problem, &plan, false, true).is_err());
        assert!(build_actions(&problem, &plan, true, false).is_err());
        assert!(build_actions(&problem, &plan, true, true).is_err());
    }

    #[test]
    fn build_activities_non_schedule() {
        let data = vec![
            (problem::mock_nontemporal(), plan::mock_nontemporal()),
            (problem::mock_temporal(), plan::mock_temporal()),
        ];
        for (problem, plan) in data {
            assert!(build_activities(&problem, &plan, false).is_err());
            assert!(build_activities(&problem, &plan, true).is_err());
        }
    }

    #[test]
    fn build_activities_schedule() -> Result<()> {
        let problem = problem::mock_schedule();
        let plan = plan::mock_schedule();

        let m = "M";
        let m1 = "M1";
        let m2 = "M2";
        let w = "W";
        let t_m = "integer[0, 1]";
        let t_w = "integer[0, 4]";

        let m1_sv = expression::state_variable(vec![
            expression::fluent_symbol_with_type(m, t_m),
            expression::parameter(m1, m),
        ]);
        let m2_sv = expression::state_variable(vec![
            expression::fluent_symbol_with_type(m, t_m),
            expression::parameter(m2, m),
        ]);
        let w_sv = expression::state_variable(vec![expression::fluent_symbol_with_type(w, t_w)]);

        let expected = vec![
            Action::Durative(DurativeAction::new(
                "a1".into(),
                "a1".into(),
                vec![],
                vec![],
                vec![
                    DurativeEffect::new(
                        m1_sv.list.clone(),
                        expression::int(1),
                        EffectKindModel::Decrease,
                        vec![],
                        Timepoint::at_start(),
                    ),
                    DurativeEffect::new(
                        w_sv.list.clone(),
                        expression::int(2),
                        EffectKindModel::Decrease,
                        vec![],
                        Timepoint::at_start(),
                    ),
                    DurativeEffect::new(
                        m1_sv.list.clone(),
                        expression::int(1),
                        EffectKindModel::Increase,
                        vec![],
                        Timepoint::at_end(),
                    ),
                    DurativeEffect::new(
                        w_sv.list.clone(),
                        expression::int(2),
                        EffectKindModel::Increase,
                        vec![],
                        Timepoint::at_end(),
                    ),
                ],
                Timepoint::fixed(20.into()),
                Timepoint::fixed(40.into()),
                Some(TemporalIntervalExpression::new(
                    expression::int(20),
                    expression::int(20),
                    false,
                    false,
                )),
            )),
            Action::Durative(DurativeAction::new(
                "a2".into(),
                "a2".into(),
                vec![],
                vec![],
                vec![
                    DurativeEffect::new(
                        m2_sv.list.clone(),
                        expression::int(1),
                        EffectKindModel::Decrease,
                        vec![],
                        Timepoint::at_start(),
                    ),
                    DurativeEffect::new(
                        w_sv.list.clone(),
                        expression::int(2),
                        EffectKindModel::Decrease,
                        vec![],
                        Timepoint::at_start(),
                    ),
                    DurativeEffect::new(
                        m2_sv.list.clone(),
                        expression::int(1),
                        EffectKindModel::Increase,
                        vec![],
                        Timepoint::at_end(),
                    ),
                    DurativeEffect::new(
                        w_sv.list.clone(),
                        expression::int(2),
                        EffectKindModel::Increase,
                        vec![],
                        Timepoint::at_end(),
                    ),
                ],
                Timepoint::fixed(0.into()),
                Timepoint::fixed(20.into()),
                Some(TemporalIntervalExpression::new(
                    expression::int(20),
                    expression::int(20),
                    false,
                    false,
                )),
            )),
        ];

        assert_eq!(build_activities(&problem, &plan, false)?, expected);
        Ok(())
    }

    #[test]
    fn test_build_goals_nontemporal() -> Result<()> {
        let p = problem::mock_nontemporal();
        let goals = build_goals(&p, false, false)?;
        assert_eq!(goals.iter().len(), 1);
        for (goal, pb_goal) in goals.iter().zip(p.goals) {
            assert!(matches!(goal, Condition::Span(_)));
            match goal {
                Condition::Span(g) => assert_eq!(g.expr(), &pb_goal.goal.unwrap()),
                Condition::Durative(_) => bail!("Expected span condition"),
            }
        }
        Ok(())
    }

    #[test]
    fn test_build_goals_temporal() -> Result<()> {
        let p = problem::mock_temporal();
        let goals = build_goals(&p, false, true)?;
        assert_eq!(goals.iter().len(), 2);
        for (i, (goal, pb_goal)) in goals.iter().zip(p.goals).enumerate() {
            if i == 0 {
                assert!(matches!(goal, Condition::Span(_)));
            } else {
                assert!(matches!(goal, Condition::Durative(_)));
            }
            match goal {
                Condition::Durative(g) => assert_eq!(g.expr(), &pb_goal.goal.unwrap()),
                Condition::Span(g) => assert_eq!(g.expr(), &pb_goal.goal.unwrap()),
            }
        }
        Ok(())
    }

    #[test]
    fn test_is_schedule() {
        let data = vec![
            (problem::mock_nontemporal(), plan::mock_nontemporal(), true, false),
            (problem::mock_nontemporal(), plan::mock_temporal(), true, false),
            (problem::mock_nontemporal(), plan::mock_schedule(), false, false),
            (problem::mock_temporal(), plan::mock_nontemporal(), true, false),
            (problem::mock_temporal(), plan::mock_temporal(), true, false),
            (problem::mock_temporal(), plan::mock_schedule(), false, false),
            (problem::mock_schedule(), plan::mock_nontemporal(), false, false),
            (problem::mock_schedule(), plan::mock_temporal(), false, false),
            (problem::mock_schedule(), plan::mock_schedule(), true, true),
        ];
        for (pb, pl, is_ok, res) in data {
            let is = is_schedule(&pb, &pl);
            assert_eq!(is.is_ok(), is_ok);
            assert_eq!(is.unwrap_or(false), res);
        }
    }

    #[test]
    fn test_is_temporal() {
        let features = [Feature::ContinuousTime, Feature::DiscreteTime];
        for (i, &feature) in features.iter().enumerate() {
            let mut p = problem::mock_nontemporal();
            assert!(!is_continuous_time(&p));
            assert!(!is_discrete_time(&p));
            assert!(!is_temporal(&p));
            p.push_features(feature);
            assert!(is_temporal(&p));
            assert_eq!(is_continuous_time(&p), i == 0);
            assert_eq!(is_discrete_time(&p), i == 1);
        }
    }
}
