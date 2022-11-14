use crate::{
    models::{env::Env, value::Value},
    traits::{interpreter::Interpreter, typeable::Typeable},
};

use anyhow::{bail, ensure, Context, Result};

pub type Procedure<E> = fn(&Env<E>, Vec<E>) -> Result<Value>;

pub fn and<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    args.iter().fold(Ok(true.into()), |r, a| match r {
        Ok(b) => b & a.eval(env)?,
        Err(e) => Err(e),
    })
}

pub fn or<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    args.iter().fold(Ok(false.into()), |r, a| match r {
        Ok(b) => b | a.eval(env)?,
        Err(e) => Err(e),
    })
}

pub fn not<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 1);
    !&args.first().unwrap().eval(env)?
}

pub fn implies<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    (!a)? | b
}

pub fn equals<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    Ok((a == b).into())
}

pub fn le<E: Interpreter + Clone>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    Ok((lt(env, args.clone())? | equals(env, args)?)?)
}

pub fn lt<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    Ok(match a {
        Value::Number(v1) => match b {
            Value::Number(v2) => (v1 < v2).into(),
            _ => bail!("<= operation with a non-number value"),
        },
        _ => bail!("<= operation with a non-number value"),
    })
}

pub fn plus<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    a + b
}

pub fn minus<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    a - b
}

pub fn times<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    a * b
}

pub fn div<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    a / b
}

pub fn exists<E: Interpreter + Typeable + Clone>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let v = args.get(0).unwrap();
    let e = args.get(1).unwrap();
    for o in env
        .get_objects(&v.tpe())
        .context(format!("No objects of type {:?}", v.tpe()))?
    {
        let mut new_env = env.clone();
        let v_name = match v.eval(env)? {
            Value::Symbol(s) => s,
            _ => bail!("Variable is not a symbol"),
        };
        new_env.bound(v.tpe().clone(), v_name, o.clone());
        if e.eval(&new_env)? == true.into() {
            return Ok(true.into());
        }
    }
    Ok(false.into())
}

pub fn forall<E: Interpreter + Typeable + Clone>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let v = args.get(0).unwrap();
    let e = args.get(1).unwrap();
    for o in env
        .get_objects(&v.tpe())
        .context(format!("No objects of type {:?}", v.tpe()))?
    {
        let mut new_env = env.clone();
        let v_name = match v.eval(env)? {
            Value::Symbol(s) => s,
            _ => bail!("Variable is not a symbol"),
        };
        new_env.bound(v.tpe().clone(), v_name, o.clone());
        if e.eval(&new_env)? == false.into() {
            return Ok(false.into());
        }
    }
    Ok(true.into())
}

#[cfg(test)]
mod tests {
    use unified_planning::{atom::Content, Atom, Expression, ExpressionKind};

    use super::*;

    #[derive(Clone, Debug)]
    struct MockExpression(Value);
    impl Default for MockExpression {
        fn default() -> Self {
            Self(true.into())
        }
    }
    impl Interpreter for MockExpression {
        fn eval(&self, _: &Env<Self>) -> Result<Value> {
            Ok(self.0.clone())
        }
    }
    impl Typeable for MockExpression {
        fn tpe(&self) -> String {
            match self.0 {
                Value::Bool(_) => "boolean".into(),
                Value::Number(_) => "number".into(),
                Value::Symbol(_) => "symbol".into(),
            }
        }
    }

    macro_rules! test_err {
        ($op:expr, $env:expr) => {
            assert!($op(&$env, vec![]).is_err());
        };
        ($op:expr, $env:expr, $($a:ident),+) => {
            assert!($op(&$env, vec![$($a.clone()),+]).is_err());
        };
    }
    macro_rules! test {
        ($op:expr, $env:expr, $e:expr, $($a:ident),+) => {
            assert_eq!($op(&$env, vec![$($a.clone()),+])?, $e.into());
        };
    }

    #[test]
    fn test_and() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let t = MockExpression(true.into());
        let f = MockExpression(false.into());
        let i = MockExpression(2.into());

        test_err!(and, env, i);
        test!(and, env, true, t);
        test!(and, env, true, t, t);
        test!(and, env, false, t, f);
        test!(and, env, false, f, t);
        test!(and, env, false, f, f);
        test!(and, env, true, t, t, t);
        test!(and, env, false, t, t, f);
        test!(and, env, false, f, f, f);
        Ok(())
    }

    #[test]
    fn test_or() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let t = MockExpression(true.into());
        let f = MockExpression(false.into());
        let i = MockExpression(2.into());

        test_err!(or, env, i);
        test!(or, env, true, t);
        test!(or, env, true, t, t);
        test!(or, env, true, t, f);
        test!(or, env, true, f, t);
        test!(or, env, false, f, f);
        test!(or, env, true, t, t, t);
        test!(or, env, true, t, t, f);
        test!(or, env, false, f, f, f);
        Ok(())
    }

    #[test]
    fn test_not() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let t = MockExpression(true.into());
        let f = MockExpression(false.into());
        let i = MockExpression(2.into());

        test_err!(not, env, i);
        test_err!(not, env);
        test_err!(not, env, t, t);
        test!(not, env, false, t);
        test!(not, env, true, f);
        Ok(())
    }

    #[test]
    fn test_implies() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let t = MockExpression(true.into());
        let f = MockExpression(false.into());
        let i = MockExpression(2.into());

        test_err!(implies, env, i);
        test_err!(implies, env);
        test_err!(implies, env, t);
        test_err!(implies, env, t, t, t);
        test!(implies, env, true, t, t);
        test!(implies, env, false, t, f);
        test!(implies, env, true, f, t);
        test!(implies, env, true, f, f);
        Ok(())
    }

    #[test]
    fn test_equals() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let t = MockExpression(true.into());
        let f = MockExpression(false.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s1 = MockExpression("a".into());
        let s2 = MockExpression("b".into());

        test_err!(equals, env);
        test_err!(equals, env, t);
        test_err!(equals, env, t, t, t);
        let values = vec![t, f, i1, i2, s1, s2];
        for i in 0..values.len() {
            for j in 0..values.len() {
                let e = i == j;
                let (v1, v2) = (values[i].clone(), values[j].clone());
                test!(equals, env, e, v1, v2);
            }
        }
        Ok(())
    }

    #[test]
    fn test_lt() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let b = MockExpression(true.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s = MockExpression("s".into());

        test_err!(lt, env);
        test_err!(lt, env, i1);
        test_err!(lt, env, i1, i1, i1);
        test_err!(lt, env, b, b);
        test_err!(lt, env, b, i1);
        test_err!(lt, env, b, s);
        test_err!(lt, env, s, b);
        test_err!(lt, env, s, i1);
        test_err!(lt, env, s, s);
        test_err!(lt, env, i1, b);
        test_err!(lt, env, i1, s);
        test!(lt, env, false, i1, i1);
        test!(lt, env, true, i1, i2);
        test!(lt, env, false, i2, i1);
        test!(lt, env, false, i2, i2);
        Ok(())
    }

    #[test]
    fn test_le() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let b = MockExpression(true.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s = MockExpression("s".into());

        test_err!(le, env);
        test_err!(le, env, i1);
        test_err!(le, env, i1, i1, i1);
        test_err!(le, env, b, b);
        test_err!(le, env, b, i1);
        test_err!(le, env, b, s);
        test_err!(le, env, s, b);
        test_err!(le, env, s, i1);
        test_err!(le, env, s, s);
        test_err!(le, env, i1, b);
        test_err!(le, env, i1, s);
        test!(le, env, true, i1, i1);
        test!(le, env, true, i1, i2);
        test!(le, env, false, i2, i1);
        test!(le, env, true, i2, i2);
        Ok(())
    }

    #[test]
    fn test_plus() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let b = MockExpression(true.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s = MockExpression("s".into());

        test_err!(plus, env);
        test_err!(plus, env, i1);
        test_err!(plus, env, i1, i1, i1);
        test_err!(plus, env, b, b);
        test_err!(plus, env, b, i1);
        test_err!(plus, env, b, s);
        test_err!(plus, env, s, b);
        test_err!(plus, env, s, i1);
        test_err!(plus, env, s, s);
        test_err!(plus, env, i1, b);
        test_err!(plus, env, i1, s);
        test!(plus, env, 6, i1, i2);
        Ok(())
    }

    #[test]
    fn test_minus() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let b = MockExpression(true.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s = MockExpression("s".into());

        test_err!(minus, env);
        test_err!(minus, env, i1);
        test_err!(minus, env, i1, i1, i1);
        test_err!(minus, env, b, b);
        test_err!(minus, env, b, i1);
        test_err!(minus, env, b, s);
        test_err!(minus, env, s, b);
        test_err!(minus, env, s, i1);
        test_err!(minus, env, s, s);
        test_err!(minus, env, i1, b);
        test_err!(minus, env, i1, s);
        test!(minus, env, -2, i1, i2);
        Ok(())
    }

    #[test]
    fn test_times() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let b = MockExpression(true.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s = MockExpression("s".into());

        test_err!(times, env);
        test_err!(times, env, i1);
        test_err!(times, env, i1, i1, i1);
        test_err!(times, env, b, b);
        test_err!(times, env, b, i1);
        test_err!(times, env, b, s);
        test_err!(times, env, s, b);
        test_err!(times, env, s, i1);
        test_err!(times, env, s, s);
        test_err!(times, env, i1, b);
        test_err!(times, env, i1, s);
        test!(times, env, 8, i1, i2);
        Ok(())
    }

    #[test]
    fn test_div() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let b = MockExpression(true.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s = MockExpression("s".into());

        test_err!(div, env);
        test_err!(div, env, i1);
        test_err!(div, env, i1, i1, i1);
        test_err!(div, env, b, b);
        test_err!(div, env, b, i1);
        test_err!(div, env, b, s);
        test_err!(div, env, s, b);
        test_err!(div, env, s, i1);
        test_err!(div, env, s, s);
        test_err!(div, env, i1, b);
        test_err!(div, env, i1, s);
        test!(div, env, 2, i2, i1);
        Ok(())
    }

    fn var() -> Expression {
        Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("o".into())),
            }),
            list: vec![],
            r#type: "t".into(),
            kind: ExpressionKind::Variable.into(),
        }
    }
    fn sv(n: &str) -> Expression {
        Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol(n.into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                var(),
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_exists() -> Result<()> {
        let mut env = Env::<Expression>::default();
        env.bound("t".into(), "o1".into(), "o1".into());
        env.bound("t".into(), "o2".into(), "o2".into());
        env.bound_fluent(vec!["f1".into(), "o1".into()], true.into());
        env.bound_fluent(vec!["f1".into(), "o2".into()], false.into());
        env.bound_fluent(vec!["f2".into(), "o1".into()], false.into());
        env.bound_fluent(vec!["f2".into(), "o2".into()], false.into());
        let var = var();
        let e1 = sv("f1");
        let e2 = sv("f2");

        test_err!(exists, env);
        test_err!(exists, env, var);
        test_err!(exists, env, var, var, var);
        test_err!(exists, env, var, e1, e2);
        test!(exists, env, true, var, e1);
        test!(exists, env, false, var, e2);
        Ok(())
    }

    #[test]
    fn test_forall() -> Result<()> {
        let mut env = Env::<Expression>::default();
        env.bound("t".into(), "o1".into(), "o1".into());
        env.bound("t".into(), "o2".into(), "o2".into());
        env.bound_fluent(vec!["f1".into(), "o1".into()], true.into());
        env.bound_fluent(vec!["f1".into(), "o2".into()], true.into());
        env.bound_fluent(vec!["f2".into(), "o1".into()], true.into());
        env.bound_fluent(vec!["f2".into(), "o2".into()], false.into());
        let var = var();
        let e1 = sv("f1");
        let e2 = sv("f2");

        test_err!(forall, env);
        test_err!(forall, env, var);
        test_err!(forall, env, var, var, var);
        test_err!(forall, env, var, e1, e2);
        test!(forall, env, true, var, e1);
        test!(forall, env, false, var, e2);
        Ok(())
    }
}
