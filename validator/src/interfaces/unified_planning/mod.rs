use std::convert::TryInto;

use anyhow::{ensure, Context, Result};
use malachite::Rational;
use unified_planning::{
    atom::Content, effect_expression::EffectKind, ActionInstance, Expression, Feature, Goal, Plan, Problem,
};

use crate::{
    models::{
        action::{Action, DurativeAction, SpanAction},
        condition::{Condition, DurativeCondition, SpanCondition},
        effects::{DurativeEffect, EffectKind as EffectKindModel, SpanEffect},
        env::Env,
        time::Timepoint,
        value::Value,
    },
    print_info, procedures,
    traits::interpreter::Interpreter,
    validate,
};

use self::{constants::*, utils::state_variable_to_signature};

mod constants;
mod expression;
#[cfg(test)]
mod factories;
mod time;
mod utils;

/*******************************************************************/

/// Validates the plan for the given UPF problem.
pub fn validate_upf(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    print_info!(verbose, "Start the validation");
    let temporal = is_temporal(problem);
    validate(
        &mut build_env(problem, verbose)?,
        &build_actions(problem, plan, verbose, temporal)?,
        &build_goals(problem, verbose, temporal)?,
        temporal,
    )
}

/*******************************************************************/

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

/*******************************************************************/

/// Builds the actions from the problem and the plan.
fn build_actions(problem: &Problem, plan: &Plan, verbose: bool, temporal: bool) -> Result<Vec<Action<Expression>>> {
    /// Creates the span or durative action.
    fn build_action(problem: &Problem, a: &ActionInstance, temporal: bool) -> Result<Action<Expression>> {
        let pb_a = &get_pb_action(problem, a)?;
        let name = pb_a.name.clone();
        let param_bounding = &build_param_bounding(pb_a, a)?;

        Ok(if temporal {
            let start = a
                .start_time
                .as_ref()
                .context("No start timepoint for a temporal action")?;
            let start = Rational::from_signeds(start.numerator, start.denominator);
            let end = a.end_time.as_ref().context("No end timepoint for a temporal action")?;
            let end = Rational::from_signeds(end.numerator, end.denominator);

            Action::Durative(DurativeAction::new(
                name,
                build_conditions_durative(pb_a, param_bounding)?,
                build_effects_durative(pb_a, param_bounding)?,
                Timepoint::fixed(start),
                Timepoint::fixed(end),
            ))
        } else {
            Action::Span(SpanAction::new(
                name,
                build_conditions_span(pb_a, param_bounding)?,
                build_effects_span(pb_a, param_bounding)?,
            ))
        })
    }

    /// Builds the conditions for a span action.
    fn build_conditions_span(
        a: &unified_planning::Action,
        param_bounding: &[(String, String, Value)],
    ) -> Result<Vec<SpanCondition<Expression>>> {
        a.conditions
            .iter()
            .map(|c| {
                Ok(SpanCondition::new(
                    c.cond.as_ref().context("Condition without expression")?.clone(),
                    param_bounding.to_owned(),
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the conditions for a durative action.
    fn build_conditions_durative(
        a: &unified_planning::Action,
        param_bounding: &[(String, String, Value)],
    ) -> Result<Vec<DurativeCondition<Expression>>> {
        let cond = build_conditions_span(a, param_bounding)?;
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
    fn build_effects_span(
        a: &unified_planning::Action,
        param_bounding: &[(String, String, Value)],
    ) -> Result<Vec<SpanEffect<Expression>>> {
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
                        vec![SpanCondition::new(cond, param_bounding.to_owned())]
                    } else {
                        vec![]
                    },
                    param_bounding.to_owned(),
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the effects for a durative action.
    fn build_effects_durative(
        a: &unified_planning::Action,
        param_bounding: &[(String, String, Value)],
    ) -> Result<Vec<DurativeEffect<Expression>>> {
        let eff = build_effects_span(a, param_bounding)?;
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
    fn build_param_bounding(
        pb_a: &unified_planning::Action,
        a: &ActionInstance,
    ) -> Result<Vec<(String, String, Value)>> {
        let mut result = Vec::new();
        let params = &a.parameters;
        let pb_params = &pb_a.parameters;
        ensure!(params.len() == pb_params.len());
        for i in 0..params.len() {
            let p = params.get(i).unwrap();
            let pb_p = pb_params.get(i).unwrap();
            let val = match p.content.as_ref().context("Atom without content")? {
                Content::Symbol(s) => s.clone().into(),
                Content::Int(i) => (*i).into(),
                Content::Real(r) => r.clone().into(),
                Content::Boolean(b) => (*b).into(),
            };
            result.push((pb_p.r#type.clone(), pb_p.name.clone(), val));
        }
        Ok(result)
    }

    /// Finds the action in the problem based on its name.
    fn get_pb_action(p: &Problem, a: &ActionInstance) -> Result<unified_planning::Action> {
        p.actions
            .iter()
            .find(|&x| x.name == a.action_name)
            .context(format!("No action named {:?} in the problem", a.action_name))
            .cloned()
    }

    /*=================================================================*/

    print_info!(verbose, "Creation of the actions");
    plan.actions
        .iter()
        .map(|a| build_action(problem, a, temporal))
        .collect::<Result<Vec<_>>>()
}

/*******************************************************************/

/// Builds the goals from the problem.
fn build_goals(problem: &Problem, verbose: bool, temporal: bool) -> Result<Vec<Condition<Expression>>> {
    /// Creates the span or durative goal.
    fn build_goal(g: &Goal, temporal: bool) -> Result<Condition<Expression>> {
        let expr = g.goal.as_ref().context("Goal without expression")?.clone();

        Ok(if temporal {
            if let Some(timing) = g.timing.clone() {
                Condition::Durative(DurativeCondition::new(expr, vec![], timing.try_into()?))
            } else {
                Condition::Span(SpanCondition::new(expr, vec![]))
            }
        } else {
            Condition::Span(SpanCondition::new(expr, vec![]))
        })
    }

    print_info!(verbose, "Creation of the goals");
    problem
        .goals
        .iter()
        .map(|g| build_goal(g, temporal))
        .collect::<Result<Vec<_>>>()
}

/*******************************************************************/

/// Returns whether or not the problem is temporal.
fn is_temporal(problem: &Problem) -> bool {
    problem.features.contains(&Feature::ContinuousTime.into())
}

/*******************************************************************/

#[cfg(test)]
mod tests {
    use anyhow::bail;

    use crate::{
        interfaces::unified_planning::factories::{expression, plan, problem},
        models::time::TemporalInterval,
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

        let mut param_bounding = Vec::new();
        param_bounding.push((robot_type.into(), robot_param.into(), r1.into()));
        param_bounding.push((loc_type.into(), "from".into(), loc1.into()));
        param_bounding.push((loc_type.into(), "to".into(), loc2.into()));

        let expected = vec![Action::Span(SpanAction::new(
            move_action.into(),
            vec![SpanCondition::new(
                expression::function_application(vec![
                    expression::function_symbol(UP_EQUALS),
                    loc_robot.clone(),
                    expression::parameter("from", loc_type),
                ]),
                param_bounding.clone(),
            )],
            vec![SpanEffect::new(
                loc_robot.list,
                expression::parameter("to", loc_type),
                EffectKindModel::Assign,
                vec![],
                param_bounding.clone(),
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

        let mut param_bounding = Vec::new();
        param_bounding.push((robot_type.into(), robot_param.into(), r1.into()));
        param_bounding.push((loc_type.into(), "from".into(), loc1.into()));
        param_bounding.push((loc_type.into(), "to".into(), loc2.into()));

        let expected = vec![Action::Durative(DurativeAction::new(
            move_action.into(),
            vec![DurativeCondition::new(
                expression::function_application(vec![
                    expression::function_symbol(UP_EQUALS),
                    loc_robot.clone(),
                    expression::parameter("from", loc_type),
                ]),
                param_bounding.clone(),
                TemporalInterval::at_start(),
            )],
            vec![
                DurativeEffect::new(
                    loc_robot.list.clone(),
                    expression::parameter(loc_u, loc_type),
                    EffectKindModel::Assign,
                    vec![],
                    Timepoint::at_start(),
                    param_bounding.clone(),
                ),
                DurativeEffect::new(
                    loc_robot.list,
                    expression::parameter("to", loc_type),
                    EffectKindModel::Assign,
                    vec![],
                    Timepoint::at_end(),
                    param_bounding.clone(),
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
