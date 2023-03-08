use crate::{
    interfaces::unified_planning::{
        constants::{UP_BOOL, UP_INTEGER, UP_REAL},
        utils::{content, fmt, state_variable_to_signature, symbol},
    },
    models::{env::Env, value::Value},
    print_expr,
    traits::{interpreter::Interpreter, typeable::Typeable},
};
use anyhow::{bail, ensure, Context, Result};
use malachite::Rational;
use unified_planning::{atom::Content, Expression, ExpressionKind, Real};

/* ========================================================================== */
/*                                 Conversion                                 */
/* ========================================================================== */

impl From<Real> for Value {
    fn from(r: Real) -> Self {
        Value::Number(Rational::from_signeds(r.numerator, r.denominator))
    }
}

impl From<Content> for Value {
    fn from(value: Content) -> Self {
        match value {
            Content::Symbol(s) => s.into(),
            Content::Int(i) => i.into(),
            Content::Real(r) => r.into(),
            Content::Boolean(b) => b.into(),
        }
    }
}

/* ========================================================================== */
/*                                 Interpreter                                */
/* ========================================================================== */

impl Interpreter for Expression {
    fn eval(&self, env: &Env<Self>) -> Result<Value> {
        let value = match self.kind() {
            ExpressionKind::Unknown => bail!("Expression without kind"),
            ExpressionKind::Constant => match content(self)? {
                Content::Symbol(s) => s.into(),
                Content::Int(i) => {
                    ensure!(self.r#type == UP_INTEGER);
                    i.into()
                }
                Content::Real(r) => {
                    ensure!(self.r#type == UP_REAL);
                    r.into()
                }
                Content::Boolean(b) => {
                    ensure!(self.r#type == UP_BOOL);
                    b.into()
                }
            },
            ExpressionKind::Parameter => {
                let s = symbol(self)?;
                env.get(&s).context(format!("Unbounded parameter {s:?}"))?.clone()
            }
            ExpressionKind::Variable => {
                let s = symbol(self)?;
                env.get(&s).unwrap_or(&s.into()).clone()
            }
            ExpressionKind::FluentSymbol => symbol(self)?.into(),
            ExpressionKind::FunctionSymbol => bail!("Cannot evaluate a function symbol"),
            ExpressionKind::StateVariable => {
                let sign = state_variable_to_signature(env, self)?;
                env.get_fluent(&sign)
                    .context(format!("Unbounded state variable {sign:?}"))?
                    .clone()
            }
            ExpressionKind::FunctionApplication => {
                let p = self
                    .list
                    .first()
                    .context("Function application without function symbol")?;
                ensure!(matches!(p.kind(), ExpressionKind::FunctionSymbol));
                let p = symbol(p)?;
                let args: Vec<_> = self.list.iter().skip(1).cloned().collect();
                env.get_procedure(&p).context(format!("Unbounded procedure {p:?}"))?(env, args)?
            }
            ExpressionKind::ContainerId => bail!("Cannot evaluate a container id"),
        };
        print_expr!(env.verbose, "{} --> \x1b[1m{:?}\x1b[0m", fmt(self, true)?, value);
        Ok(value)
    }
}

/* ========================================================================== */
/*                                  Typeable                                  */
/* ========================================================================== */

impl Typeable for Expression {
    fn tpe(&self) -> String {
        self.r#type.clone()
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use crate::interfaces::unified_planning::factories::expression;

    use super::*;

    fn vb(b: bool) -> Value {
        b.into()
    }
    fn vs(s: &str) -> Value {
        s.into()
    }

    #[test]
    fn value_from_real() {
        let real = Real {
            numerator: 5,
            denominator: 2,
        };
        let rational = Rational::from_signeds(5, 2);
        assert_eq!(Value::Number(rational), real.into());
    }

    #[test]
    fn eval_unknown() {
        let env = Env::default();
        let e = expression::unknown();
        assert!(e.eval(&env).is_err());
    }

    #[test]
    fn eval_constant() -> Result<()> {
        let env = Env::default();
        let s = expression::symbol("s", "t");
        let i = expression::int(2);
        let mut i_invalid = i.clone();
        i_invalid.r#type = UP_BOOL.into();
        let r = expression::real(6, 2);
        let mut r_invalid = r.clone();
        r_invalid.r#type = UP_BOOL.into();
        let b = expression::boolean(true);
        let mut b_invalid = b.clone();
        b_invalid.r#type = UP_INTEGER.into();

        assert_eq!(s.eval(&env)?, vs("s"));
        assert_eq!(i.eval(&env)?, 2.into());
        assert_eq!(r.eval(&env)?, 3.into());
        assert_eq!(b.eval(&env)?, true.into());
        assert!(i_invalid.eval(&env).is_err());
        assert!(r_invalid.eval(&env).is_err());
        assert!(b_invalid.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_parameter() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "p".into(), vb(true));
        let param = expression::parameter("p", "t");
        let unbound = expression::parameter("u", "t");
        let invalid = expression::atom(Content::Int(2), "", ExpressionKind::Parameter);
        assert_eq!(param.eval(&env)?, vb(true));
        assert!(unbound.eval(&env).is_err());
        assert!(invalid.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_variable() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "v".into(), vb(true));
        let param = expression::variable("t", "v");
        let unbound = expression::variable("t", "u");
        let invalid = expression::atom(Content::Int(2), "", ExpressionKind::Variable);
        assert_eq!(param.eval(&env)?, vb(true));
        assert_eq!(unbound.eval(&env)?, vs("u"));
        assert!(invalid.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_fluent_symbol() -> Result<()> {
        let env = Env::default();
        let e = expression::fluent_symbol("s");
        assert_eq!(e.eval(&env)?, "s".into());
        Ok(())
    }

    #[test]
    fn eval_function_symbol() {
        let env = Env::default();
        let e = expression::function_symbol("s");
        assert!(e.eval(&env).is_err());
    }

    #[test]
    fn eval_state_variable() -> Result<()> {
        let mut env = Env::default();
        env.bound_fluent(vec![vs("loc"), vs("R1")], vs("L3"));
        env.bound("r".into(), "R1".into(), vs("R1"));
        let expr = expression::state_variable(vec![expression::fluent_symbol("loc"), expression::parameter("R1", "r")]);
        let unbound = expression::state_variable(vec![expression::fluent_symbol("pos")]);
        let invalid = expression::state_variable(vec![
            expression::parameter("loc", "l"),
            expression::parameter("R1", "r"),
        ]);
        let empty = expression::state_variable(vec![]);
        assert_eq!(expr.eval(&env)?, vs("L3"));
        assert!(unbound.eval(&env).is_err());
        assert!(invalid.eval(&env).is_err());
        assert!(empty.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_function_application() -> Result<()> {
        fn proc(env: &Env<Expression>, args: Vec<Expression>) -> Result<Value> {
            let a1 = args.get(0).unwrap().eval(env)?;
            let a2 = args.get(1).unwrap().eval(env)?;
            (!a1)? & a2
        }

        let mut env = Env::default();
        env.bound_procedure("p".into(), proc);
        let expr = expression::function_application(vec![
            expression::function_symbol("p"),
            expression::boolean(false),
            expression::boolean(true),
        ]);
        let unbound = expression::function_application(vec![expression::function_symbol("and")]);
        let invalid = expression::function_application(vec![
            expression::parameter("p", "t"),
            expression::boolean(false),
            expression::boolean(true),
        ]);
        let empty = expression::function_application(vec![]);
        assert_eq!(expr.eval(&env)?, vb(true));
        assert!(unbound.eval(&env).is_err());
        assert!(invalid.eval(&env).is_err());
        assert!(empty.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_container_id() {
        let env = Env::default();
        let e = expression::container_id();
        assert!(e.eval(&env).is_err());
    }

    #[test]
    fn tpe() {
        let s = expression::symbol("s", "t");
        let i = expression::int(2);
        let r = expression::real(6, 2);
        let b = expression::boolean(true);
        assert_eq!(s.tpe(), "t".to_string());
        assert_eq!(i.tpe(), UP_INTEGER.to_string());
        assert_eq!(r.tpe(), UP_REAL.to_string());
        assert_eq!(b.tpe(), UP_BOOL.to_string());
    }
}
