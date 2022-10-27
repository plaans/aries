extern crate core;

use anyhow::*;
use malachite::Rational;
use std::collections::HashMap;
use std::ops::{Add, Div, Mul, Not, Sub};
use unified_planning::atom::Content;
use unified_planning::*;

/********************************************************************
 * VALIDATION                                                       *
 ********************************************************************/

pub fn validate(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    if verbose {
        println!("\x1b[95m[INFO]\x1b[0m Creation of the initial state");
    }
    let mut env = Env::build_initial(problem, verbose)?;
    let mut state = State::build_initial(problem, &env)?;
    env.state = state.clone();
    if verbose {
        println!("\x1b[95m[INFO]\x1b[0m Simulation of the actions");
    }
    for action in plan.actions.iter() {
        state = state.apply_action(&env, action)?;
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

impl Mul for Value {
    type Output = Result<Self>;

    fn mul(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(n1) => match rhs {
                Value::Number(n2) => Ok(Value::Number(n1 * n2)),
                _ => bail!("The value must be a number"),
            },
            _ => bail!("The value must be a number"),
        }
    }
}

impl Div for Value {
    type Output = Result<Self>;

    fn div(self, rhs: Self) -> Self::Output {
        match self {
            Value::Number(n1) => match rhs {
                Value::Number(n2) => Ok(Value::Number(n1 / n2)),
                _ => bail!("The value must be a number"),
            },
            _ => bail!("The value must be a number"),
        }
    }
}

impl Not for Value {
    type Output = Result<Self>;

    fn not(self) -> Self::Output {
        match self {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            _ => bail!("The value must be a boolean"),
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
}

/********************************************************************
 * STATE                                                            *
 ********************************************************************/

#[derive(Clone, Debug)]
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
            .context(format!("Signature {:?} not found in the state", sign.sign))
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
        if !check_conditions(&new_env, conditions)? {
            bail!("All the conditions are not verified");
        }
        let effects = action
            .effects
            .iter()
            .map(|e| e.effect.as_ref().context("Effect without expression"))
            .collect::<Result<_>>()?;
        let changes = effects_changes(&new_env, effects)?;
        let changes = changes.iter().filter_map(|c| c.as_ref()).collect::<Vec<_>>();
        let mut changed_sign = changes.iter().map(|(s, _)| s).collect::<Vec<_>>();
        changed_sign.sort_unstable();
        changed_sign.dedup();
        if changed_sign.len() != changes.len() {
            bail!("A state variable is changed by two different effects");
        } else {
            let mut state = self.clone();
            for (sign, val) in changes {
                state.assign(sign, val.clone());
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

fn effect_change(env: &Env, effect: &EffectExpression) -> Result<Option<(Signature, Value)>> {
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
    if change_value {
        let value = effect.value.as_ref().context("No value in the effect")?.eval(env)?;
        let value = match effect.kind() {
            effect_expression::EffectKind::Assign => value,
            effect_expression::EffectKind::Increase => (env.get_state_var(&sign)? + value)?,
            effect_expression::EffectKind::Decrease => (env.get_state_var(&sign)? - value)?,
        };
        Ok(Some((sign, value)))
    } else {
        Ok(None)
    }
}

fn effects_changes(env: &Env, effects: Vec<&EffectExpression>) -> Result<Vec<Option<(Signature, Value)>>> {
    Ok(effects.iter().map(|e| effect_change(env, e)).collect::<Result<_>>()?)
}

/********************************************************************
 * PROCEDURES                                                       *
 ********************************************************************/

type Procedure = fn(&Env, &[Expression]) -> Result<Value>;

fn and(env: &Env, args: &[Expression]) -> Result<Value> {
    let mut result = true;
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    for arg in args {
        result &= match arg {
            Value::Bool(b) => b,
            _ => bail!("Cannot apply a logical and to a non boolean value"),
        };
    }
    Ok(Value::Bool(result))
}

fn or(env: &Env, args: &[Expression]) -> Result<Value> {
    let mut result = false;
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    for arg in args {
        result |= match arg {
            Value::Bool(b) => b,
            _ => bail!("Cannot apply a logical or to a non boolean value"),
        };
    }
    Ok(Value::Bool(result))
}

fn not(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 1);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v = args.first().context("No argument for the 'not' procedure")?;
    !v.clone()
}

fn implies(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args
        .get(0)
        .context("Not enough arguments for 'implies' procedure")?
        .clone();
    let v2 = args
        .get(1)
        .context("Not enough arguments for 'implies' procedure")?
        .clone();
    // A implies B  <==>  (not A) or B
    match v1 {
        Value::Bool(b1) => match v2 {
            Value::Bool(b2) => Ok(Value::Bool(!b1 || b2)),
            _ => bail!("Cannot apply 'implies' procedure to a non boolean value"),
        },
        _ => bail!("Cannot apply 'implies' procedure to a non boolean value"),
    }
}

fn equals(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args.get(0).context("Not enough arguments for 'equals' procedure")?;
    let v2 = args.get(1).context("Not enough arguments for 'equals' procedure")?;
    Ok(Value::Bool(v1 == v2))
}

fn le(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args.get(0).context("Not enough arguments for 'le' procedure")?;
    let v2 = args.get(1).context("Not enough arguments for 'le' procedure")?;
    match v1 {
        Value::Number(r1) => match v2 {
            Value::Number(r2) => Ok(Value::Bool(r1 <= r2)),
            _ => bail!("Cannot compare a non number value"),
        },
        _ => bail!("Cannot compare a non number value"),
    }
}

fn plus(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args.get(0).context("Not enough arguments for 'plus' procedure")?;
    let v2 = args.get(1).context("Not enough arguments for 'plus' procedure")?;
    v1.clone() + v2.clone()
}

fn minus(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args.get(0).context("Not enough arguments for 'minus' procedure")?;
    let v2 = args.get(1).context("Not enough arguments for 'minus' procedure")?;
    v1.clone() - v2.clone()
}

fn times(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args.get(0).context("Not enough arguments for 'times' procedure")?;
    let v2 = args.get(1).context("Not enough arguments for 'times' procedure")?;
    v1.clone() * v2.clone()
}

fn div(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let args: Vec<Value> = args.iter().map(|e| e.eval(env)).collect::<Result<_>>()?;
    let v1 = args.get(0).context("Not enough arguments for 'div' procedure")?;
    let v2 = args.get(1).context("Not enough arguments for 'div' procedure")?;
    v1.clone() / v2.clone()
}

fn exists(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let var = args.get(0).context("Malformed 'exists' procedure")?;
    let exp = args.get(1).context("Malformed 'exists' procedure")?;
    ensure!(matches!(var.get_kind()?, ExpressionKind::Variable));
    let var_name = match var.content()? {
        Content::Symbol(s) => s,
        _ => bail!("Malformed variable"),
    };
    for object in env.get_objects(var.r#type.as_str())? {
        let mut new_env = env.clone();
        new_env.vars.insert(var_name.clone(), object);
        if check_condition(&new_env, exp)? {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

fn forall(env: &Env, args: &[Expression]) -> Result<Value> {
    ensure!(args.len() == 2);
    let var = args.get(0).context("Malformed 'forall' procedure")?;
    let exp = args.get(1).context("Malformed 'forall' procedure")?;
    ensure!(matches!(var.get_kind()?, ExpressionKind::Variable));
    let var_name = match var.content()? {
        Content::Symbol(s) => s,
        _ => bail!("Malformed variable"),
    };
    for object in env.get_objects(var.r#type.as_str())? {
        let mut new_env = env.clone();
        new_env.vars.insert(var_name.clone(), object);
        if !check_condition(&new_env, exp)? {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

/********************************************************************
 * ENVIRONMENT                                                      *
 ********************************************************************/

#[derive(Clone)]
struct Env {
    verbose: bool,
    state: State,
    vars: HashMap<String, Value>,
    procedures: HashMap<String, Procedure>,
    fluent_defaults: HashMap<String, Value>,
    actions: HashMap<String, Action>,
    objects: HashMap<String, Vec<Value>>,
}

impl Env {
    fn build_initial(problem: &Problem, verbose: bool) -> Result<Self> {
        let state = State::empty();
        let vars = problem
            .objects
            .iter()
            .map(|o| (o.name.clone(), Value::Sym(o.name.clone())))
            .collect();
        let objects = problem.objects.iter().fold(
            HashMap::new(),
            |mut init: HashMap<String, Vec<Value>>, item: &ObjectDeclaration| {
                let tpe = item.r#type.clone();
                let value = Value::Sym(item.name.clone());
                let new_vec: Vec<Value> = init
                    .remove(&tpe)
                    .map(|mut val| {
                        val.push(value.clone());
                        val
                    })
                    .unwrap_or_else(|| vec![value])
                    .to_vec();
                init.insert(tpe, new_vec);
                init
            },
        );
        let actions = problem.actions.iter().map(|a| (a.name.clone(), a.clone())).collect();
        let procedures: HashMap<String, Procedure> = HashMap::from([
            ("up:and".to_string(), and as Procedure),
            ("up:or".to_string(), or as Procedure),
            ("up:not".to_string(), not as Procedure),
            ("up:implies".to_string(), implies as Procedure),
            ("up:equals".to_string(), equals as Procedure),
            ("up:le".to_string(), le as Procedure),
            ("up:plus".to_string(), plus as Procedure),
            ("up:minus".to_string(), minus as Procedure),
            ("up:times".to_string(), times as Procedure),
            ("up:div".to_string(), div as Procedure),
            ("up:exists".to_string(), exists as Procedure),
            ("up:forall".to_string(), forall as Procedure),
        ]);
        let mut env = Env {
            verbose,
            state,
            vars,
            procedures,
            fluent_defaults: HashMap::new(),
            actions,
            objects,
        };
        for f in problem.fluents.iter() {
            if let Some(default) = &f.default_value {
                let value = default.eval(&env)?;
                env.fluent_defaults.insert(f.name.clone(), value);
            }
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

    fn get_objects(&self, tpe: &str) -> Result<Vec<Value>> {
        Ok(self
            .objects
            .get(tpe)
            .context(format!("No objects of type {:?}", tpe))?
            .to_vec())
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
        if env.verbose {
            println!("\x1b[96m[Expr]\x1b[0m {:?}", self)
        }
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
                let sym = self.list.first().context("No function symbol")?;
                ensure!(matches!(sym.get_kind()?, ExpressionKind::FunctionSymbol));
                let procedure = match sym.content()? {
                    Content::Symbol(s) => env.get_proc(s),
                    _ => bail!("Malformed function application"),
                }?;
                let args: Vec<Expression> = self.list.iter().skip(1).cloned().collect();
                procedure(env, &args)
            }
            ExpressionKind::ContainerId => bail!("Container id cannot be evaluated individually"),
        }
    }
}
