use std::convert::TryInto;

use anyhow::{Context, Result};
use unified_planning::{Expression, Problem};

use crate::{models::env::Env, procedures, traits::interpreter::Interpreter};

use super::{constants::*, utils::state_variable_to_signature};

impl TryInto<Env<Expression>> for Problem {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Env<Expression>, Self::Error> {
        let mut env = Env::default();

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
        env.bound_procedure(UP_LE.into(), procedures::le);
        env.bound_procedure(UP_PLUS.into(), procedures::plus);
        env.bound_procedure(UP_MINUS.into(), procedures::minus);
        env.bound_procedure(UP_TIMES.into(), procedures::times);
        env.bound_procedure(UP_DIV.into(), procedures::div);
        env.bound_procedure(UP_EXISTS.into(), procedures::exists);
        env.bound_procedure(UP_FORALL.into(), procedures::forall);

        // Returns the environment.
        Ok(env)
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
        e.bound_procedure(UP_LE.into(), procedures::le);
        e.bound_procedure(UP_PLUS.into(), procedures::plus);
        e.bound_procedure(UP_MINUS.into(), procedures::minus);
        e.bound_procedure(UP_TIMES.into(), procedures::times);
        e.bound_procedure(UP_DIV.into(), procedures::div);
        e.bound_procedure(UP_EXISTS.into(), procedures::exists);
        e.bound_procedure(UP_FORALL.into(), procedures::forall);

        assert_eq!(e, p.try_into()?);
        Ok(())
    }
}
