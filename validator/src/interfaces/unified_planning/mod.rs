use std::{collections::HashMap, convert::TryInto, ops::Deref};

use crate::{
    models::{
        action::{Action, DurativeAction, SpanAction},
        condition::{Condition, DurativeCondition, SpanCondition},
        effects::{DurativeEffect, EffectKind as EffectKindModel, SpanEffect},
        env::Env,
        method::{Method, Subtask},
        parameter::Parameter,
        task::{Refiner, Task},
        time::{TemporalInterval, Timepoint},
    },
    print_info, procedures,
    traits::interpreter::Interpreter,
    validate,
};
use anyhow::{bail, ensure, Context, Result};
use malachite::Rational;
use unified_planning::{
    effect_expression::EffectKind, ActionInstance, Expression, Feature, Goal, Hierarchy, Plan, PlanHierarchy, Problem,
};

use self::{constants::*, utils::state_variable_to_signature};

mod constants;
mod expression;
#[cfg(test)]
mod factories;
mod time;
mod utils;

/* ========================================================================== */
/*                               Entry Function                               */
/* ========================================================================== */

/// Validates the plan for the given UPF problem.
pub fn validate_upf(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    check_is_supported_problem(problem)?;
    print_info!(verbose, "Start the validation");
    let temporal = is_temporal(problem);
    let actions = build_actions(problem, plan, verbose, temporal)?;
    validate(
        &mut build_env(problem, verbose)?,
        &actions,
        build_root_tasks(problem, plan, &Action::into_durative(&actions), verbose)?.as_ref(),
        &build_goals(problem, verbose, temporal)?,
        temporal,
    )
}

/* ========================================================================== */
/*                             Supported Problems                             */
/* ========================================================================== */

/// Checks that the problem kind is supported.
fn check_is_supported_problem(problem: &Problem) -> Result<()> {
    let supported_features = vec![
        // Problem class
        Feature::ActionBased,
        Feature::Hierarchical,
        // Problem type
        Feature::SimpleNumericPlanning,
        Feature::GeneralNumericPlanning,
        // Time
        Feature::ContinuousTime,
        Feature::DiscreteTime,
        Feature::IntermediateConditionsAndEffects,
        Feature::TimedEffect,
        Feature::TimedGoals,
        Feature::DurationInequalities,
        // Expression duration
        Feature::StaticFluentsInDuration,
        Feature::FluentsInDuration,
        // Numbers
        Feature::ContinuousNumbers,
        Feature::DiscreteNumbers,
        // Conditions kind
        Feature::NegativeConditions,
        Feature::DisjunctiveConditions,
        Feature::Equality,
        Feature::ExistentialConditions,
        Feature::UniversalConditions,
        // Effects kind
        Feature::ConditionalEffects,
        Feature::IncreaseEffects,
        Feature::DecreaseEffects,
        // Typing
        Feature::FlatTyping,
        Feature::HierarchicalTyping,
        // Fluents type
        Feature::NumericFluents,
        Feature::ObjectFluents,
        // Quality metrics
        Feature::ActionsCost,
        Feature::FinalValue,
        Feature::Makespan,
        Feature::PlanLength,
        Feature::Oversubscription,
        // Hierarchical
        Feature::MethodPreconditions,
        Feature::TaskNetworkConstraints,
        Feature::InitialTaskNetworkVariables,
        Feature::TaskOrderTotal,
        Feature::TaskOrderPartial,
        Feature::TaskOrderTemporal,
    ];
    for feature in &problem.features {
        if !supported_features.contains(&Feature::from_i32(*feature).unwrap()) {
            bail!("Unsupported feature");
        }
    }
    Ok(())
}

/* ========================================================================== */
/*                             Environment Factory                            */
/* ========================================================================== */

/// Builds the initial environment from the problem.
fn build_env(problem: &Problem, verbose: bool) -> Result<Env<Expression>> {
    print_info!(verbose, "Creation of the initial state");
    let mut env = Env::default();
    env.verbose = verbose;

    // Bounds types.
    for t in problem.types.iter() {
        env.bound_type(t.type_name.clone(), t.parent_type.clone());
    }

    // Bounds objects.
    for o in problem.objects.iter() {
        env.bound(o.r#type.clone(), o.name.clone(), o.name.clone().into());
    }

    // Bounds fluents of the initial state.
    for assignment in problem.initial_state.iter() {
        let k = state_variable_to_signature(&env, assignment.fluent.as_ref().context("Assignment without fluent")?)?;
        let v = assignment
            .value
            .as_ref()
            .context("Assignment without value")?
            .eval(&env)?;
        env.bound_fluent(k, v);
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

    // Returns the environment.
    Ok(env)
}

/* ========================================================================== */
/*                               Actions Factory                              */
/* ========================================================================== */

/// Builds the actions from the problem and the plan.
fn build_actions(problem: &Problem, plan: &Plan, verbose: bool, temporal: bool) -> Result<Vec<Action<Expression>>> {
    /* =========================== Utils Functions ========================== */

    /// Creates the span or durative action.
    fn build_action(problem: &Problem, a: &ActionInstance, temporal: bool) -> Result<Action<Expression>> {
        let pb_a = &get_pb_action(problem, a)?;

        Ok(if temporal {
            let start = a
                .start_time
                .as_ref()
                .context("No start timepoint for a temporal action")?;
            let start = Rational::from_signeds(start.numerator, start.denominator);
            let end = a.end_time.as_ref().context("No end timepoint for a temporal action")?;
            let end = Rational::from_signeds(end.numerator, end.denominator);

            Action::Durative(DurativeAction::new(
                a.action_name.clone(),
                a.id.clone(),
                build_params(pb_a, a)?,
                build_conditions_durative(pb_a)?,
                build_effects_durative(pb_a)?,
                Timepoint::fixed(start),
                Timepoint::fixed(end),
            ))
        } else {
            Action::Span(SpanAction::new(
                a.action_name.clone(),
                a.id.clone(),
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
                Ok(DurativeCondition::from_span(
                    s,
                    c.span
                        .clone()
                        .context("Durative condition without temporal interval")?
                        .try_into()?,
                ))
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
                Ok(DurativeEffect::from_span(
                    s,
                    e.occurrence_time
                        .clone()
                        .context("Durative effect without occurrence time")?
                        .try_into()?,
                ))
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
    plan.actions
        .iter()
        .map(|a| build_action(problem, a, temporal))
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
        let task = subtasks.iter().find(|t| t.id == task_id.to_string()).context(format!(
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
                        .find(|a| a.name() == &task.task_name)
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
            .find(|m| m.id == id.to_string())
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
                    TemporalInterval::overall()
                };
                Ok(DurativeCondition::from_span(span, interval))
            })
            .collect::<Result<Vec<_>>>()?;

        let constraints = pb_meth
            .constraints
            .iter()
            .map(|c| DurativeCondition::new(c.clone(), TemporalInterval::overall()))
            .collect::<Vec<_>>();

        Ok(Method::new(
            pb_meth.name.to_string(),
            id.to_string(),
            params,
            conditions.into_iter().chain(constraints.into_iter()).collect(),
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
                    build_subtask(&pb_hierarchy, &plan_hierarchy, actions, subtasks, task_id, refiner_id)?,
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
/*                          Problem and Plan Features                         */
/* ========================================================================== */

/// Returns whether or not the problem is temporal.
fn is_temporal(problem: &Problem) -> bool {
    problem.features.contains(&Feature::ContinuousTime.into())
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use anyhow::bail;

    use crate::{
        interfaces::unified_planning::factories::{expression, plan, problem},
        models::{parameter::Parameter, time::TemporalInterval},
    };

    use super::*;

    #[test]
    fn test_build_env() -> Result<()> {
        let p = problem::mock_nontemporal();
        let mut e = Env::<Expression>::default();

        // Types
        e.bound_type("locatable".into(), "".into());
        e.bound_type("robot".into(), "locatable".into());
        e.bound_type("location".into(), "locatable".into());

        // Objects
        e.bound("robot".into(), "R1".into(), "R1".into());
        e.bound("location".into(), "L1".into(), "L1".into());
        e.bound("location".into(), "L2".into(), "L2".into());

        // Fluents
        e.bound_fluent(vec!["loc".into(), "R1".into()], "L1".into());

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

        assert_eq!(e, build_env(&p, false)?);
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
        ))];

        assert_eq!(build_actions(&problem, &plan, false, true)?, expected);
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
    fn test_is_temporal() {
        let mut p = problem::mock_nontemporal();
        assert!(!is_temporal(&p));
        p.push_features(Feature::ContinuousTime);
        assert!(is_temporal(&p));
    }
}
