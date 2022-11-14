use std::convert::{TryFrom, TryInto};

use anyhow::{Context, Result};
use unified_planning::{Expression, Problem};

use crate::{
    models::{
        env::Env,
        goal::{Goal, GoalIter},
    },
    procedures,
    traits::interpreter::Interpreter,
};

use super::{constants::*, utils::state_variable_to_signature};

impl TryInto<Env<Expression>> for Problem {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Env<Expression>, Self::Error> {
        let mut env = Env::default();

        // Bounds types.
        for t in self.types.iter() {
            env.bound_type(t.type_name.clone(), t.parent_type.clone());
        }

        // Bounds objects.
        for o in self.objects.iter() {
            env.bound(o.r#type.clone(), o.name.clone(), o.name.clone().into());
        }

        // Bounds fluents of the initial state.
        for assignment in self.initial_state.iter() {
            let k =
                state_variable_to_signature(&env, assignment.fluent.as_ref().context("Assignment without fluent")?)?;
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
}

impl TryFrom<Problem> for GoalIter<Expression> {
    type Error = anyhow::Error;

    fn try_from(value: Problem) -> Result<Self, Self::Error> {
        let goals = value
            .goals
            .iter()
            .map(|g| Ok(Goal::from(g.goal.as_ref().context("Goal without expression")?.clone())))
            .collect::<Result<Vec<_>>>()?;
        Ok(GoalIter::from(goals))
    }
}

#[cfg(test)]
mod tests {
    use crate::interfaces::unified_planning::factories::ProblemFactory;

    use super::*;

    #[test]
    fn into_env() -> Result<()> {
        let p = ProblemFactory::mock();
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

        assert_eq!(e, p.try_into()?);
        Ok(())
    }

    #[test]
    fn try_from() -> Result<()> {
        let p = ProblemFactory::mock();
        let goals = GoalIter::try_from(p.clone())?;
        assert_eq!(goals.iter().len(), 1);
        for (goal, pb_goal) in goals.iter().zip(p.goals) {
            assert_eq!(goal.expr(), &pb_goal.goal.unwrap());
        }
        Ok(())
    }
}
