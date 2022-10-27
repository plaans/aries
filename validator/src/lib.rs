extern crate core;

use anyhow::*;
use malachite::Rational;
use std::collections::HashMap;
use std::ops::{Add, Sub};
use unified_planning::atom::Content;
use unified_planning::*;

/********************************************************************
 * VALIDATION                                                       *
 ********************************************************************/

pub fn validate(problem: &Problem, plan: &Plan) -> Result<()> {
    let mut env = Env::build_initial(problem)?;
    let mut state = State::build_initial(problem, &env)?;
    for action in plan.actions.iter() {
        state = state.apply_action(&env, &action)?;
        env.state = state.clone();
    }
    Ok(())
}

/********************************************************************
 * TYPES                                                            *
 ********************************************************************/

const UP_BOOL: &str = "up:bool";
const UP_INTEGER: &str = "up:integer";
const UP_REAL: &str = "up:real";
const UP_SYMBOL: &str = "up:symbol";

/********************************************************************
 * VALUE                                                            *
 ********************************************************************/

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Value {
    Bool(bool),
    Number(Rational),
    Sym(String),
}

impl Add for Value {
    type Output = Result<Self>;

    fn add(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(n1) => match rhs {
                Value::Number(n2) => Ok(Value::Number(n1 + n2)),
                _ => bail!("The value must be a number"),
            },
            _ => bail!("The value must be a number"),
        }
    }
}

impl Sub for Value {
    type Output = Result<Self>;

    fn sub(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(n1) => match rhs {
                Value::Number(n2) => Ok(Value::Number(n1 - n2)),
                _ => bail!("The value must be a number"),
            },
            _ => bail!("The value must be a number"),
        }
    }
}

/********************************************************************
 * SIGNATURE                                                        *
 ********************************************************************/

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Signature {
    sign: Vec<Value>,
}

impl Signature {
    fn new(sign: Vec<Value>) -> Self {
        Signature { sign }
    }

    fn head(&self) -> Result<&Value> {
        self.sign.first().context("No head in the signature")
    }

    fn args(&self) -> Vec<&Value> {
        self.sign.iter().skip(1).collect()
    }
}

/********************************************************************
 * STATE                                                            *
 ********************************************************************/

#[derive(Clone)]
struct State {
    vars: HashMap<Signature, Value>,
}

impl State {
    fn build_initial(problem: &Problem, env: &Env) -> Result<Self> {
        let mut state = State::empty();
        for assignment in problem.initial_state.iter() {
            let fluent = assignment.fluent.as_ref().context("No fluent in the assignment")?;
            ensure!(matches!(fluent.get_kind()?, ExpressionKind::StateVariable));
            let value = assignment
                .value
                .as_ref()
                .context("No value in the assignment")?
                .eval(env)?;
            state.assign(&fluent.signature(env)?, value);
        }
        Ok(state)
    }

    fn empty() -> Self {
        State { vars: HashMap::new() }
    }

    fn assign(&mut self, sign: &Signature, val: Value) {
        self.vars.insert(sign.clone(), val);
    }

    fn get_var(&self, sign: &Signature) -> Result<Value> {
        self.vars
            .get(sign)
            .context(format!("Signature {:?} not found in the state", sign))
            .cloned()
    }

    fn apply_action(&self, env: &Env, action_impl: &ActionInstance) -> Result<Self> {
        let action = env.get_action(action_impl.action_name.as_str())?;
        let mut new_env = env.clone();
        new_env.extend_with_action(&action, action_impl)?;
        let conditions = action
            .conditions
            .iter()
            .map(|c| c.cond.as_ref().context("Condition without expression"))
            .collect::<Result<_>>()?;
        check_conditions(&new_env, conditions)?;
        let effects = action
            .effects
            .iter()
            .map(|e| e.effect.as_ref().context("Effect without expression"))
            .collect::<Result<_>>()?;
        let changes = effects_changes(&new_env, effects)?;
        let mut changed_sign = changes.iter().map(|(s, _)| s).collect::<Vec<_>>();
        changed_sign.sort_unstable();
        changed_sign.dedup();
        if changed_sign.len() != changes.len() {
            bail!("A state variable is changed by two different effects");
        } else {
            let mut state = self.clone();
            for (sign, val) in changes {
                state.assign(&sign, val);
            }
            Ok(state)
        }
    }
}

fn check_condition(env: &Env, condition: &Expression) -> Result<bool> {
    Ok(condition.eval(env)? == Value::Bool(true))
}

fn check_conditions(env: &Env, conditions: Vec<&Expression>) -> Result<bool> {
    Ok(conditions
        .iter()
        .map(|c| check_condition(env, c))
        .collect::<Result<Vec<bool>>>()?
        .iter()
        .all(|&x| x))
}

fn effect_change(env: &Env, effect: &EffectExpression) -> Result<(Signature, Value)> {
    let change_value = if let Some(up_condition) = &effect.condition {
        check_condition(env, up_condition)?
    } else {
        true
    };
    let sign = effect
        .fluent
        .as_ref()
        .context("No fluent in the effect")?
        .signature(env)?;
    let value = if change_value {
        let value = effect.value.as_ref().context("No value in the effect")?.eval(env)?;
        match effect.kind() {
            effect_expression::EffectKind::Assign => value,
            effect_expression::EffectKind::Increase => (env.get_state_var(&sign)? + value)?,
            effect_expression::EffectKind::Decrease => (env.get_state_var(&sign)? - value)?,
        }
    } else {
        env.get_state_var(&sign)?
    };
    Ok((sign, value))
}

fn effects_changes(env: &Env, effects: Vec<&EffectExpression>) -> Result<Vec<(Signature, Value)>> {
    Ok(effects.iter().map(|e| effect_change(env, e)).collect::<Result<_>>()?)
}

/********************************************************************
 * ENVIRONMENT                                                      *
 ********************************************************************/

type Procedure = fn(&[Value]) -> Result<Value>;

#[derive(Clone)]
struct Env {
    state: State,
    vars: HashMap<String, Value>,
    procedures: HashMap<String, Procedure>,
    fluent_defaults: HashMap<String, Value>,
    actions: HashMap<String, Action>,
}

impl Env {
    fn build_initial(problem: &Problem) -> Result<Self> {
        let state = State::empty();
        let vars = problem
            .objects
            .iter()
            .map(|o| (o.name.clone(), Value::Sym(o.name.clone())))
            .collect();
        let actions = problem.actions.iter().map(|a| (a.name.clone(), a.clone())).collect();
        let mut env = Env {
            state,
            vars,
            procedures: HashMap::new(),
            fluent_defaults: HashMap::new(),
            actions,
        };
        for f in problem.fluents.iter() {
            let value = f
                .default_value
                .as_ref()
                .context(format!("No default value for the fluent {:?}", f))?
                .eval(&env)?;
            env.fluent_defaults.insert(f.name.clone(), value);
        }
        Ok(env)
    }

    fn get_proc(&self, s: &str) -> Result<Procedure> {
        self.procedures
            .get(s)
            .context(format!("No procedure called {:?}", s))
            .cloned()
    }

    fn get_state_var(&self, sign: &Signature) -> Result<Value> {
        let result = self.state.get_var(sign);
        if result.is_err() {
            match sign.head()? {
                Value::Sym(s) => Ok(self
                    .fluent_defaults
                    .get(s)
                    .context(format!("No default value for the fluent {:?}", s))?
                    .clone()),
                _ => bail!("Malformed state variable signature"),
            }
        } else {
            result
        }
    }

    fn get_var(&self, s: &str) -> Result<Value> {
        self.vars.get(s).context(format!("Unbound variable {:?}", s)).cloned()
    }

    fn get_action(&self, a: &str) -> Result<Action> {
        self.actions.get(a).context(format!("No action named {:?}", a)).cloned()
    }

    fn extend_with_action(&mut self, action: &Action, action_impl: &ActionInstance) -> Result<()> {
        let values = action_impl
            .parameters
            .iter()
            .map(atom_to_expr)
            .collect::<Result<Vec<_>>>()?
            .iter()
            .map(|e| e.eval(self))
            .collect::<Result<Vec<_>>>()?;
        self.vars.extend(
            action
                .parameters
                .iter()
                .map(|p| p.name.clone())
                .zip(values)
                .collect::<HashMap<_, _>>(),
        );
        Ok(())
    }
}

fn atom_to_expr(atom: &Atom) -> Result<Expression> {
    Ok(unified_planning::Expression {
        atom: Some(atom.clone()),
        list: vec![],
        r#type: match atom.content.as_ref().context("No content in the atom")? {
            Content::Symbol(_) => UP_SYMBOL.to_string(),
            Content::Int(_) => UP_INTEGER.to_string(),
            Content::Real(_) => UP_REAL.to_string(),
            Content::Boolean(_) => UP_BOOL.to_string(),
        },
        kind: ExpressionKind::Constant.into(),
    })
}

/********************************************************************
 * EXPRESSION                                                       *
 ********************************************************************/

trait ExtExpr {
    fn content(&self) -> Result<&Content>;
    fn get_kind(&self) -> Result<ExpressionKind>;
    fn signature(&self, env: &Env) -> Result<Signature>;
    fn eval(&self, env: &Env) -> Result<Value>;
}

impl ExtExpr for Expression {
    fn content(&self) -> Result<&Content> {
        let a = self.atom.as_ref().context("No atom in the expression")?;
        match a.content.as_ref() {
            Some(c) => Ok(c),
            _ => bail!("No content in the atom of the expression"),
        }
    }

    fn get_kind(&self) -> Result<ExpressionKind> {
        ExpressionKind::from_i32(self.kind).context(format!("Unknown expression kind id: {}", self.kind))
    }

    fn signature(&self, env: &Env) -> Result<Signature> {
        let sign: Vec<Value> = self.list.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
        Ok(Signature::new(sign))
    }

    fn eval(&self, env: &Env) -> Result<Value> {
        match self.get_kind()? {
            ExpressionKind::Unknown => {
                bail!("Expression kind not specified in protobuf")
            }
            ExpressionKind::Constant => match self.content()? {
                Content::Symbol(s) => env.get_var(s),
                Content::Int(i) => {
                    ensure!(self.r#type == UP_INTEGER);
                    Ok(Value::Number(Rational::from(*i)))
                }
                Content::Real(r) => {
                    ensure!(self.r#type == UP_REAL);
                    Ok(Value::Number(Rational::from_signeds(r.numerator, r.denominator)))
                }
                Content::Boolean(b) => {
                    ensure!(self.r#type == UP_BOOL);
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
                let sign = self.signature(env)?;
                ensure!(matches!(sign.head()?, Value::Sym(_)));
                env.get_state_var(&sign)
            }
            ExpressionKind::FunctionSymbol => bail!("Function symbol cannot be evaluated individually"),
            ExpressionKind::FunctionApplication => {
                let sign = self.signature(env)?;
                let procedure = match sign.head()? {
                    Value::Sym(s) => env.get_proc(s),
                    _ => bail!("Malformed function application signature"),
                }?;
                let args: Vec<Value> = sign.args().iter().map(|&x| x.clone()).collect();
                procedure(&args)
            }
            ExpressionKind::ContainerId => bail!("Container id cannot be evaluated individually"),
        }
    }
}
