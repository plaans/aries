use crate::models::{env::Env, value::Value};
use crate::traits::interpreter::Interpreter;
use anyhow::{bail, ensure, Context, Result};
use std::fmt::Write;
use unified_planning::{atom::Content, Expression, ExpressionKind};

/// Returns the content of the atom of the expression.
pub fn content(e: &Expression) -> Result<Content> {
    e.atom
        .clone()
        .context("Expression without atom")?
        .content
        .context("Atom without content")
}

/// Returns the symbols stored in the content of the expression.
pub fn symbol(e: &Expression) -> Result<String> {
    match content(e)? {
        Content::Symbol(s) => Ok(s),
        _ => bail!("Expression without symbol"),
    }
}

/// Converts the state variable to a value signature.
pub fn state_variable_to_signature(env: &Env<Expression>, e: &Expression) -> Result<Vec<Value>> {
    ensure!(matches!(e.kind(), ExpressionKind::StateVariable));
    let f = e.list.first().context("State variable without fluent symbol")?;
    ensure!(matches!(f.kind(), ExpressionKind::FluentSymbol));
    let f = symbol(f)?.into();
    let mut args: Vec<_> = e.list.iter().skip(1).map(|a| a.eval(env)).collect::<Result<_>>()?;
    args.insert(0, f);
    Ok(args)
}

/// Formats the expression in a more human-readable way.
///
/// If `kind = true` then print the kind of the expression.
/// This is useful for recursive calls.
pub fn fmt(e: &Expression, kind: bool) -> Result<String> {
    let mut s = String::new();
    if kind {
        write!(s, "{:?} ", e.kind())?;
    }
    match e.kind() {
        ExpressionKind::Constant => write!(s, "{:?}", content(e)?),
        ExpressionKind::Parameter | ExpressionKind::Variable => write!(s, "{} - {}", symbol(e)?, e.r#type),
        ExpressionKind::FluentSymbol | ExpressionKind::FunctionSymbol => write!(s, "{}", symbol(e)?),
        ExpressionKind::StateVariable | ExpressionKind::FunctionApplication => {
            write!(s, "{}", fmt(e.list.first().context("Missing head")?, false)?)?;
            let args = e
                .list
                .iter()
                .skip(1)
                .map(|a| fmt(a, false))
                .collect::<Result<Vec<_>>>()?;
            if !args.is_empty() {
                write!(s, " ( ")?;
                for i in 0..args.len() {
                    write!(s, "{}", args.get(i).unwrap())?;
                    if i != args.len() - 1 {
                        write!(s, " , ")?;
                    }
                }
                write!(s, " )")
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }?;
    Ok(s)
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use crate::interfaces::unified_planning::factories::expression;

    use super::*;

    #[test]
    fn test_content() -> Result<()> {
        let c = Content::Symbol("o".into());
        let e1 = expression::atom(c.clone(), "t".into(), ExpressionKind::Constant.into());
        let mut e2 = e1.clone();
        e2.atom.as_mut().unwrap().content = None;
        let mut e3 = e1.clone();
        e3.atom = None;

        assert_eq!(content(&e1)?, c);
        assert!(content(&e2).is_err());
        assert!(content(&e3).is_err());
        Ok(())
    }

    #[test]
    fn test_symbol() -> Result<()> {
        let s = expression::symbol("o", "t");
        let i = expression::int(2);
        let r = expression::real(6, 2);
        let b = expression::boolean(true);

        assert_eq!(symbol(&s)?, "o".to_string());
        assert!(symbol(&i).is_err());
        assert!(symbol(&r).is_err());
        assert!(symbol(&b).is_err());
        Ok(())
    }

    #[test]
    fn test_state_variable_to_signature() -> Result<()> {
        let mut env = Env::<Expression>::default();
        env.bound("t".into(), "p".into(), 2.into());
        let sv = expression::state_variable(vec![expression::fluent_symbol("f"), expression::parameter("p", "t")]);
        let no_sv = expression::parameter("p", "t");
        let no_f = expression::state_variable(vec![expression::parameter("p", "t")]);
        let empty = expression::state_variable(vec![]);

        assert_eq!(state_variable_to_signature(&env, &sv)?, vec!["f".into(), 2.into()]);
        assert!(state_variable_to_signature(&env, &no_sv).is_err());
        assert!(state_variable_to_signature(&env, &no_f).is_err());
        assert!(state_variable_to_signature(&env, &empty).is_err());
        Ok(())
    }
}
