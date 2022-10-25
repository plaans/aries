extern crate core;

use anyhow::*;
use malachite::Rational;
use std::collections::HashMap;
use unified_planning::atom::Content;
use unified_planning::*;

/********************************************************************
 * MAIN SECTION                                                     *
 ********************************************************************/

fn main() {
    println!("COUCOU"); // TODO
}

fn validate(problem: &Problem, plan: &Plan) -> Result<()> {
    Ok(()) // TODO
}

/********************************************************************
 * VALUE                                                            *
 ********************************************************************/

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Value {
    Bool(bool),
    Number(malachite::Rational),
    Sym(String),
}

/********************************************************************
 * STATE                                                            *
 ********************************************************************/

#[derive(Debug, PartialEq, Eq, Hash)]
struct Signature {
    sign: Vec<Value>,
}

impl Signature {
    fn new(sign: Vec<Value>) -> Self {
        Signature { sign }
    }
}

struct State {
    vars: HashMap<Signature, Value>,
}

impl State {
    fn get_var(&self, sign: &Signature) -> Result<Value> {
        self.vars
            .get(sign)
            .context(format!("Signature {:?} not found in the state", sign))
            .cloned()
    }
}

fn build_initial_state(problem: &Problem) -> State {
    todo!() // TODO
}

fn resulting_state(state: State, eff: &EffectExpression) -> State {
    todo!() // TODO
}

/********************************************************************
 * ENVIRONMENT                                                      *
 ********************************************************************/

type Procedure = fn(&[Value]) -> Result<Value>;

struct Env {
    state: State,
    vars: HashMap<String, Value>,
    procedures: HashMap<String, Procedure>,
}

impl Env {
    fn get_proc(&self, s: &str) -> Result<Procedure> {
        self.procedures
            .get(s)
            .context(format!("No procedure called {:?}", s))
            .cloned()
    }

    fn get_state_var(&self, sign: &Signature) -> Result<Value> {
        self.state.get_var(sign)
    }

    fn get_var(&self, s: &str) -> Result<Value> {
        self.vars.get(s).context(format!("Unbound variable {:?}", s)).cloned()
    }
}

/********************************************************************
 * EXPRESSION                                                       *
 ********************************************************************/

struct Expression<'a> {
    up_expr: &'a unified_planning::Expression,
}

impl<'a> Expression<'a> {
    fn from_up(e: &'a unified_planning::Expression) -> Self {
        Self { up_expr: e }
    }

    fn content(&self) -> Result<&atom::Content> {
        let a = self.up_expr.atom.as_ref().context("No atom in the expression")?;
        match a.content.as_ref() {
            Some(c) => Ok(c),
            _ => bail!("No content in the atom of the expression"),
        }
    }

    fn kind(&self) -> Result<ExpressionKind> {
        ExpressionKind::from_i32(self.up_expr.kind)
            .context(format!("Unknown expression kind id: {}", self.up_expr.kind))
    }

    fn sub_expressions(&self) -> Vec<Expression> {
        self.up_expr.list.iter().map(Expression::from_up).collect()
    }

    fn eval(&self, env: &Env) -> Result<Value> {
        match self.kind()? {
            ExpressionKind::Unknown => {
                bail!("Expression kind not specified in protobuf")
            }
            ExpressionKind::Constant => match self.content()? {
                Content::Symbol(s) => env.get_var(s),
                Content::Int(i) => {
                    ensure!(self.up_expr.r#type == "up:integer");
                    Ok(Value::Number(Rational::from(*i)))
                }
                Content::Real(r) => {
                    ensure!(self.up_expr.r#type == "up:real");
                    Ok(Value::Number(Rational::from_signeds(r.numerator, r.denominator)))
                }
                Content::Boolean(b) => {
                    ensure!(self.up_expr.r#type == "up:bool");
                    Ok(Value::Bool(*b))
                }
            },
            ExpressionKind::Parameter | ExpressionKind::Variable => match self.content()? {
                Content::Symbol(s) => env.get_var(s),
                _ => bail!("Malformed expression"),
            },
            ExpressionKind::FluentSymbol => match self.content()? {
                Content::Symbol(s) => Ok(Value::Sym(s.clone())),
                _ => bail!("Malformed expression"),
            },
            ExpressionKind::StateVariable => {
                let sub_e = self.sub_expressions();
                let fluent = sub_e.first().context("List is empty for a state variable")?;
                ensure!(fluent.kind()? == ExpressionKind::FluentSymbol);
                let sign: Vec<Value> = sub_e.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
                env.get_state_var(&Signature::new(sign))
            }
            ExpressionKind::FunctionSymbol => bail!("Function symbol cannot be evaluated individually"),
            ExpressionKind::FunctionApplication => {
                let mut sub_e = self.sub_expressions();
                let procedure = sub_e.pop().context("List is empty for a function application")?;
                ensure!(procedure.kind()? == ExpressionKind::FunctionSymbol);
                let procedure = match procedure.content()? {
                    Content::Symbol(s) => env.get_proc(s),
                    _ => bail!("Malformed expression"),
                }?;
                let args: Vec<Value> = sub_e.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
                procedure(&args)
            }
            ExpressionKind::ContainerId => bail!("Container id cannot be evaluated individually"),
        }
    }
}
