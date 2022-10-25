extern crate core;

use anyhow::*;
use malachite::Rational;
use std::collections::HashMap;
use unified_planning::atom::Content;
use unified_planning::*;

fn main() {
    println!("COUCOU");
}

fn validate(problem: &Problem, plan: &Plan) -> Result<()> {
    Ok(())
}

struct State {
    // map([fluent, param1: Value, parm2: Value, ...] -> Value
}

#[derive(Clone)]
enum Value {
    Bool(bool),
    Number(malachite::Rational),
    Sym(String),
}

struct Env {
    state: State,
    vars: HashMap<String, Value>,
}

fn build_initial_state(problem: &Problem) -> State {
    todo!()
}

fn eval_boolean_condition(expression: &Expression) -> bool {
    todo!()
}

fn content(e: &Expression) -> Result<&atom::Content> {
    let a = e.atom.as_ref().context("No atom un boolean expression")?;
    match a.content.as_ref() {
        Some(c) => Ok(c),
        _ => bail!("No content in expression"),
    }
}

fn eval_expression(env: &Env, e: &Expression) -> Result<Value> {
    match kind(e)? {
        ExpressionKind::Unknown => {
            bail!("Expression kind not specified in protobuf")
        }
        ExpressionKind::Constant => match content(e)? {
            Content::Symbol(_) => {
                todo!()
            }
            Content::Int(i) => {
                ensure!(e.r#type == "up:integer");
                Ok(Value::Number(Rational::from(*i)))
            }
            Content::Real(_) => {
                todo!()
            }
            Content::Boolean(b) => {
                ensure!(e.r#type == "up:bool");
                Ok(Value::Bool(*b))
            }
        },
        ExpressionKind::Parameter | ExpressionKind::Variable => match content(e)? {
            Content::Symbol(s) => env.vars.get(s).context(format!("Unbound variable {:?}", &s)).cloned(),
            _ => bail!("Malformed expression"),
        },
        _ => todo!(), // ExpressionKind::Variable => {}
                      // ExpressionKind::FluentSymbol => {}
                      // ExpressionKind::FunctionSymbol => {}
                      // ExpressionKind::StateVariable => {}
                      // ExpressionKind::FunctionApplication => {}
                      // ExpressionKind::ContainerId => {}
    }
}

fn resulting_state(state: State, eff: &EffectExpression) -> State {
    todo!()
}

fn kind(e: &Expression) -> Result<ExpressionKind, Error> {
    ExpressionKind::from_i32(e.kind).with_context(|| format!("Unknown expression kind id: {}", e.kind))
}
