use std::convert::TryFrom;

use anyhow::{ensure, Context, Result};
use unified_planning::{atom::Content, ActionInstance, Expression, Plan, Problem};

use crate::models::{
    action::{Action, ActionIter},
    condition::Condition,
    effects::{Effect, EffectKind},
    env::Env,
};

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
fn get_conditions(a: &unified_planning::Action) -> Result<Vec<Condition<Expression>>> {
    a.conditions
        .iter()
        .map(|c| {
            Ok(Condition::from(
                c.cond.as_ref().context("Condition without expression")?.clone(),
            ))
        })
        .collect::<Result<Vec<_>>>()
}

/// Builds the effects for the action.
fn get_effects(a: &unified_planning::Action) -> Result<Vec<Effect<Expression>>> {
    a.effects
        .iter()
        .map(|e| {
            let expr = e.effect.as_ref().context("Effect without expression")?;
            Ok(Effect::new(
                expr.clone().fluent.context("Effect without fluent")?.list,
                expr.clone().value.context("Effect without value")?,
                match expr.clone().kind() {
                    unified_planning::effect_expression::EffectKind::Assign => EffectKind::Assign,
                    unified_planning::effect_expression::EffectKind::Increase => EffectKind::Increase,
                    unified_planning::effect_expression::EffectKind::Decrease => EffectKind::Decrease,
                },
                if let Some(cond) = expr.clone().condition {
                    vec![Condition::from(cond)]
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

impl TryFrom<(Problem, Plan)> for ActionIter<Expression> {
    type Error = anyhow::Error;

    fn try_from(value: (Problem, Plan)) -> Result<Self, Self::Error> {
        let (problem, plan) = value;
        let actions = plan
            .actions
            .iter()
            .map(|a| {
                let pb_a = get_pb_action(&problem, a)?;
                Ok(Action::new(
                    pb_a.name.clone(),
                    get_conditions(&pb_a)?,
                    get_effects(&pb_a)?,
                    build_local_env(&pb_a, a)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ActionIter::from(actions))
    }
}

#[cfg(test)]
mod tests {
    use crate::interfaces::unified_planning::{
        constants::UP_EQUALS,
        factories::{ExpressionFactory, PlanFactory, ProblemFactory},
    };

    use super::*;

    #[test]
    fn try_from() -> Result<()> {
        let problem = ProblemFactory::mock();
        let plan = PlanFactory::mock();

        let robot_type = "robot";
        let robot_param = "r";
        let r1 = "R1";
        let loc_type = "location";
        let loc_fluent = "loc";
        let loc1 = "L1";
        let loc2 = "L2";
        let move_action = "move";

        let loc_robot = ExpressionFactory::state_variable(vec![
            ExpressionFactory::fluent_symbol(loc_fluent),
            ExpressionFactory::parameter(robot_param, robot_type),
        ]);

        let mut local_env = Env::default();
        local_env.bound(robot_type.into(), robot_param.into(), r1.into());
        local_env.bound(loc_type.into(), "from".into(), loc1.into());
        local_env.bound(loc_type.into(), "to".into(), loc2.into());

        let expected = ActionIter::from(vec![Action::new(
            move_action.into(),
            vec![Condition::from(ExpressionFactory::function_application(vec![
                ExpressionFactory::function_symbol(UP_EQUALS),
                loc_robot.clone(),
                ExpressionFactory::parameter("from", loc_type),
            ]))],
            vec![Effect::new(
                loc_robot.list,
                ExpressionFactory::parameter("to", loc_type),
                EffectKind::Assign,
                vec![],
            )],
            local_env,
        )]);

        assert_eq!(ActionIter::try_from((problem, plan))?, expected);
        Ok(())
    }
}
