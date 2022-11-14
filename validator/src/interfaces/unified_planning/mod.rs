use anyhow::{ensure, Context, Result};
use unified_planning::{atom::Content, effect_expression::EffectKind, ActionInstance, Expression, Plan, Problem};

use crate::{
    models::{
        action::Action as ActionModel,
        condition::Condition as ConditionModel,
        effects::{Effect as EffectModel, EffectKind as EffectKindModel},
        env::Env,
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
mod utils;

/// Validates the plan for the given UPF problem.
pub fn validate_upf(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    print_info!(verbose, "Start the validation");
    validate(
        &mut build_env(problem, verbose)?,
        build_actions(problem, plan, verbose)?.iter(),
        build_goals(problem, verbose)?.iter(),
    )
}

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

/// Builds the actions from the problem and the plan.
fn build_actions(problem: &Problem, plan: &Plan, verbose: bool) -> Result<Vec<ActionModel<Expression>>> {
    /// Creates the environment to map the Action to its Instance.
    fn build_local_env(pb_a: &unified_planning::Action, a: &ActionInstance) -> Result<Env<Expression>> {
        let mut env = Env::default();
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
            env.bound(pb_p.r#type.clone(), pb_p.name.clone(), val);
        }
        Ok(env)
    }

    /// Builds the conditions for the action.
    fn get_conditions(a: &unified_planning::Action) -> Result<Vec<ConditionModel<Expression>>> {
        a.conditions
            .iter()
            .map(|c| {
                Ok(ConditionModel::from(
                    c.cond.as_ref().context("Condition without expression")?.clone(),
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Builds the effects for the action.
    fn get_effects(a: &unified_planning::Action) -> Result<Vec<EffectModel<Expression>>> {
        a.effects
            .iter()
            .map(|e| {
                let expr = e.effect.as_ref().context("Effect without expression")?;
                Ok(EffectModel::new(
                    expr.clone().fluent.context("Effect without fluent")?.list,
                    expr.clone().value.context("Effect without value")?,
                    match expr.clone().kind() {
                        EffectKind::Assign => EffectKindModel::Assign,
                        EffectKind::Increase => EffectKindModel::Increase,
                        EffectKind::Decrease => EffectKindModel::Decrease,
                    },
                    if let Some(cond) = expr.clone().condition {
                        vec![ConditionModel::from(cond)]
                    } else {
                        vec![]
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Finds the action in the problem based on its name.
    fn get_pb_action(p: &Problem, a: &ActionInstance) -> Result<unified_planning::Action> {
        p.actions
            .iter()
            .find(|&x| x.name == a.action_name)
            .context(format!("No action named {:?} in the problem", a.action_name))
            .cloned()
    }

    print_info!(verbose, "Creation of the actions");
    plan.actions
        .iter()
        .map(|a| {
            let pb_a = get_pb_action(problem, a)?;
            Ok(ActionModel::new(
                pb_a.name.clone(),
                get_conditions(&pb_a)?,
                get_effects(&pb_a)?,
                build_local_env(&pb_a, a)?,
            ))
        })
        .collect::<Result<Vec<_>>>()
}

/// Builds the goals from the problem.
fn build_goals(problem: &Problem, verbose: bool) -> Result<Vec<ConditionModel<Expression>>> {
    print_info!(verbose, "Creation of the goals");
    problem
        .goals
        .iter()
        .map(|g| {
            Ok(ConditionModel::from(
                g.goal.as_ref().context("Goal without expression")?.clone(),
            ))
        })
        .collect::<Result<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use crate::interfaces::unified_planning::factories::{expression, plan, problem};

    use super::*;

    #[test]
    fn test_build_env() -> Result<()> {
        let p = problem::mock();
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
    fn test_build_actions() -> Result<()> {
        let problem = problem::mock();
        let plan = plan::mock();

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

        let mut local_env = Env::default();
        local_env.bound(robot_type.into(), robot_param.into(), r1.into());
        local_env.bound(loc_type.into(), "from".into(), loc1.into());
        local_env.bound(loc_type.into(), "to".into(), loc2.into());

        let expected = vec![ActionModel::new(
            move_action.into(),
            vec![ConditionModel::from(expression::function_application(vec![
                expression::function_symbol(UP_EQUALS),
                loc_robot.clone(),
                expression::parameter("from", loc_type),
            ]))],
            vec![EffectModel::new(
                loc_robot.list,
                expression::parameter("to", loc_type),
                EffectKindModel::Assign,
                vec![],
            )],
            local_env,
        )];

        assert_eq!(build_actions(&problem, &plan, false)?, expected);
        Ok(())
    }

    #[test]
    fn test_build_goals() -> Result<()> {
        let p = problem::mock();
        let goals = build_goals(&p, false)?;
        assert_eq!(goals.iter().len(), 1);
        for (goal, pb_goal) in goals.iter().zip(p.goals) {
            assert_eq!(goal.expr(), &pb_goal.goal.unwrap());
        }
        Ok(())
    }
}
