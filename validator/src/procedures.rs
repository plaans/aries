use crate::{
    models::{env::Env, value::Value},
    traits::{durative::Durative, interpreter::Interpreter, typeable::Typeable},
};

use anyhow::{bail, ensure, Context, Result};

/* ========================================================================== */
/*                                 Procedures                                 */
/* ========================================================================== */

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
    lt(env, args.clone())? | equals(env, args)?
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

pub fn exists<E: Clone + Interpreter + Typeable>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
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

pub fn forall<E: Interpreter + Clone + Typeable>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
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

pub fn iff<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    Ok(match a {
        Value::Bool(v1) => match b {
            Value::Bool(v2) => (v1 == v2).into(),
            _ => bail!("iff procedure with a non-boolean value"),
        },
        _ => bail!("iff procedure with a non-boolean value"),
    })
}

pub fn end<E: Interpreter + std::fmt::Debug>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 1);
    let id = args.first().unwrap().eval(env)?;
    let id = match id {
        Value::Symbol(s) => s,
        _ => bail!(format!("Expected a symbol but got {id}")),
    };

    if let Some(method) = env.crt_method() {
        if let Some(subtask) = method.subtasks().get(&id) {
            Ok(subtask.end(env).eval(Some(method), env).into())
        } else {
            bail!(format!("No subtask with the id {id}"));
        }
    } else {
        bail!(format!(
            "No method in the current environment, cannot evaluate subtask {id}"
        ));
    }
}

pub fn start<E: Interpreter>(env: &Env<E>, args: Vec<E>) -> Result<Value> {
    ensure!(args.len() == 1);
    let id = args.first().unwrap().eval(env)?;
    let id = match id {
        Value::Symbol(s) => s,
        _ => bail!(format!("Expected a symbol but got {id}")),
    };

    if let Some(method) = env.crt_method() {
        if let Some(subtask) = method.subtasks().get(&id) {
            Ok(subtask.start(env).eval(Some(method), env).into())
        } else {
            bail!(format!("No subtask with the id {id}"));
        }
    } else {
        bail!(format!(
            "No method in the current environment, cannot evaluate subtask {id}"
        ));
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use unified_planning::{atom::Content, Atom, Expression, ExpressionKind};

    use crate::models::{
        action::DurativeAction,
        method::{Method, Subtask},
        time::Timepoint,
    };

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

        fn convert_to_csp_constraint(&self, _: &Env<Self>) -> Result<crate::models::csp::CspConstraint> {
            todo!();
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
            assert!($op(&$env, vec![]).is_err())
        };
        ($op:expr, $env:expr, $($a:ident),+) => {
            assert!($op(&$env, vec![$($a.clone()),+]).is_err())
        };
    }
    macro_rules! test {
        ($op:expr, $env:expr, $e:expr, $($a:ident),+) => {
            assert_eq!($op(&$env, vec![$($a.clone()),+])?, $e.into())
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

    fn var(n: &str, t: &str) -> Expression {
        Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol(n.into())),
            }),
            list: vec![],
            r#type: t.into(),
            kind: ExpressionKind::Variable.into(),
        }
    }
    fn sv(n: &str, t: &str) -> Expression {
        Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol(n.into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                var("o", t),
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
        env.bound_fluent(vec!["f1".into(), "o1".into()], true.into())?;
        env.bound_fluent(vec!["f1".into(), "o2".into()], false.into())?;
        env.bound_fluent(vec!["f2".into(), "o1".into()], false.into())?;
        env.bound_fluent(vec!["f2".into(), "o2".into()], false.into())?;
        let var = var("o", "t");
        let e1 = sv("f1", "t");
        let e2 = sv("f2", "t");

        test_err!(exists, env);
        test_err!(exists, env, var);
        test_err!(exists, env, var, var, var);
        test_err!(exists, env, var, e1, e2);
        test!(exists, env, true, var, e1);
        test!(exists, env, false, var, e2);
        Ok(())
    }

    #[test]
    fn test_exists_double() -> Result<()> {
        let mut env = Env::<Expression>::default();
        env.bound("t1".into(), "o11".into(), "o11".into());
        env.bound("t1".into(), "o12".into(), "o12".into());
        env.bound("t2".into(), "o21".into(), "o21".into());
        env.bound("t2".into(), "o22".into(), "o22".into());
        env.bound_fluent(vec!["f".into(), "o11".into(), "o21".into()], true.into())?;
        env.bound_fluent(vec!["f".into(), "o11".into(), "o22".into()], false.into())?;
        env.bound_fluent(vec!["f".into(), "o12".into(), "o21".into()], false.into())?;
        env.bound_fluent(vec!["f".into(), "o12".into(), "o22".into()], false.into())?;
        env.bound_procedure("exists".into(), exists);
        let expr = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("exists".into())),
                    }),
                    kind: ExpressionKind::FunctionSymbol.into(),
                    ..Default::default()
                },
                var("o2", "t2"),
                Expression {
                    list: vec![
                        Expression {
                            atom: Some(Atom {
                                content: Some(Content::Symbol("f".into())),
                            }),
                            kind: ExpressionKind::FluentSymbol.into(),
                            ..Default::default()
                        },
                        var("o1", "t1"),
                        var("o2", "t2"),
                    ],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::FunctionApplication.into(),
            ..Default::default()
        };
        let var = var("o1", "t1");

        test!(exists, env, true, var, expr);
        env.bound_fluent(vec!["f".into(), "o11".into(), "o21".into()], false.into())?;
        test!(exists, env, false, var, expr);
        Ok(())
    }

    #[test]
    fn test_forall() -> Result<()> {
        let mut env = Env::<Expression>::default();
        env.bound("t".into(), "o1".into(), "o1".into());
        env.bound("t".into(), "o2".into(), "o2".into());
        env.bound_fluent(vec!["f1".into(), "o1".into()], true.into())?;
        env.bound_fluent(vec!["f1".into(), "o2".into()], true.into())?;
        env.bound_fluent(vec!["f2".into(), "o1".into()], true.into())?;
        env.bound_fluent(vec!["f2".into(), "o2".into()], false.into())?;
        let var = var("o", "t");
        let e1 = sv("f1", "t");
        let e2 = sv("f2", "t");

        test_err!(forall, env);
        test_err!(forall, env, var);
        test_err!(forall, env, var, var, var);
        test_err!(forall, env, var, e1, e2);
        test!(forall, env, true, var, e1);
        test!(forall, env, false, var, e2);
        Ok(())
    }

    #[test]
    fn test_forall_double() -> Result<()> {
        let mut env = Env::<Expression>::default();
        env.bound("t1".into(), "o11".into(), "o11".into());
        env.bound("t1".into(), "o12".into(), "o12".into());
        env.bound("t2".into(), "o21".into(), "o21".into());
        env.bound("t2".into(), "o22".into(), "o22".into());
        env.bound_fluent(vec!["f".into(), "o11".into(), "o21".into()], true.into())?;
        env.bound_fluent(vec!["f".into(), "o11".into(), "o22".into()], true.into())?;
        env.bound_fluent(vec!["f".into(), "o12".into(), "o21".into()], true.into())?;
        env.bound_fluent(vec!["f".into(), "o12".into(), "o22".into()], true.into())?;
        env.bound_procedure("forall".into(), forall);
        let expr = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("forall".into())),
                    }),
                    kind: ExpressionKind::FunctionSymbol.into(),
                    ..Default::default()
                },
                var("o2", "t2"),
                Expression {
                    list: vec![
                        Expression {
                            atom: Some(Atom {
                                content: Some(Content::Symbol("f".into())),
                            }),
                            kind: ExpressionKind::FluentSymbol.into(),
                            ..Default::default()
                        },
                        var("o1", "t1"),
                        var("o2", "t2"),
                    ],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::FunctionApplication.into(),
            ..Default::default()
        };
        let var = var("o1", "t1");

        test!(forall, env, true, var, expr);
        env.bound_fluent(vec!["f".into(), "o11".into(), "o21".into()], false.into())?;
        test!(forall, env, false, var, expr);
        Ok(())
    }

    #[test]
    fn test_iff() -> Result<()> {
        let env = Env::<MockExpression>::default();
        let t = MockExpression(true.into());
        let f = MockExpression(false.into());
        let i1 = MockExpression(2.into());
        let i2 = MockExpression(4.into());
        let s1 = MockExpression("a".into());
        let s2 = MockExpression("b".into());

        test_err!(iff, env);
        test_err!(iff, env, t);
        test_err!(iff, env, t, t, t);
        let values = vec![t, f, i1, i2, s1, s2];
        for i in 0..values.len() {
            for j in 0..values.len() {
                let (v1, v2) = (values[i].clone(), values[j].clone());
                let (e1, e2) = (v1.eval(&env)?, v2.eval(&env)?);

                match e1 {
                    Value::Bool(_) => match e2 {
                        Value::Bool(_) => test!(iff, env, i == j, v1, v2),
                        _ => test_err!(iff, env, v1, v2),
                    },
                    _ => test_err!(iff, env, v1, v2),
                };
            }
        }
        Ok(())
    }

    fn _build_container_env() -> Env<MockExpression> {
        fn a(n: &str, s: Timepoint, e: Timepoint) -> DurativeAction<MockExpression> {
            DurativeAction::new(n.into(), n.into(), vec![], vec![], vec![], s, e, None)
        }
        fn st_a(n: &str, s: Timepoint, e: Timepoint) -> Subtask<MockExpression> {
            Subtask::Action(a(n, s, e))
        }
        fn m(n: &str, st: HashMap<String, Subtask<MockExpression>>) -> Method<MockExpression> {
            Method::new(n.into(), n.into(), vec![], vec![], vec![], st)
        }
        fn t(i: i32) -> Timepoint {
            Timepoint::fixed(i.into())
        }

        let mut env = Env::<MockExpression>::default();
        env.global_end = 302.into();
        let s1 = t(0);
        let e1 = t(100);
        let a1 = st_a("a1", s1.clone(), e1);
        let s2 = t(101);
        let e2 = t(251);
        let a2 = st_a("a2", s2, e2);
        let s3 = t(252);
        let e3 = t(302);
        let a3 = st_a("a3", s3, e3.clone());
        let mth = m(
            "m",
            HashMap::from([("s1".into(), a1), ("s2".into(), a2), ("s3".into(), a3)]),
        );
        env.set_method(mth);
        env
    }

    #[test]
    fn test_end() -> Result<()> {
        let mut env = _build_container_env();

        // Valid arguments
        let ids = ["s1", "s2", "s3"]
            .iter()
            .map(|&s| MockExpression(s.into()))
            .collect::<Vec<_>>();
        let expected = &[100, 251, 302];
        for (id, &ex) in ids.iter().zip(expected) {
            test!(end, env, ex, id);
            test_err!(end, env);
            test_err!(end, env, id, id);
        }

        // Invalid arguments
        let fails = &[
            MockExpression("s4".into()),
            MockExpression(true.into()),
            MockExpression(15.into()),
        ];
        for f in fails.into_iter() {
            test_err!(end, env, f);
        }

        // No method
        env.clear_method();
        for id in ids.iter() {
            test_err!(end, env, id);
            test_err!(end, env);
            test_err!(end, env, id, id);
        }

        Ok(())
    }

    #[test]
    fn test_start() -> Result<()> {
        let mut env = _build_container_env();

        // Valid arguments
        let ids = ["s1", "s2", "s3"]
            .iter()
            .map(|&s| MockExpression(s.into()))
            .collect::<Vec<_>>();
        let expected = &[0, 101, 252];
        for (id, &ex) in ids.iter().zip(expected) {
            test!(start, env, ex, id);
            test_err!(start, env);
            test_err!(start, env, id, id);
        }

        // Invalid arguments
        let fails = &[
            MockExpression("s4".into()),
            MockExpression(true.into()),
            MockExpression(15.into()),
        ];
        for f in fails.into_iter() {
            test_err!(start, env, f);
        }

        // No method
        env.clear_method();
        for id in ids.iter() {
            test_err!(start, env, id);
            test_err!(start, env);
            test_err!(start, env, id, id);
        }

        Ok(())
    }
}
