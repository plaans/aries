//! Values read from the Python objects of the unified-planning library.
//!
//! Expressions are mirrored with the shape of their protobuf counterpart (as produced by
//! `unified_planning.grpc.proto_writer`) so that `convert.rs` can follow `chronicles.rs` of the gRPC server.

use anyhow::{bail, Result};
use itertools::Itertools;
use num_rational::Ratio;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Real {
    pub numerator: i64,
    pub denominator: i64,
}

impl From<Ratio<i64>> for Real {
    fn from(value: Ratio<i64>) -> Self {
        Real {
            numerator: *value.numer(),
            denominator: *value.denom(),
        }
    }
}

pub mod atom {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Content {
        Symbol(String),
        Int(i64),
        Real(super::Real),
        Boolean(bool),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    pub content: Option<atom::Content>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExpressionKind {
    Constant,
    Parameter,
    Variable,
    FluentSymbol,
    FunctionSymbol,
    StateVariable,
    FunctionApplication,
    ContainerId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expression {
    pub atom: Option<Atom>,
    pub list: Vec<Expression>,
    pub r#type: String,
    pub kind: ExpressionKind,
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(atom) = &self.atom {
            match atom.content.as_ref().unwrap() {
                atom::Content::Symbol(s) => write!(f, "{s}"),
                atom::Content::Int(i) => write!(f, " {i}"),
                atom::Content::Real(r) => {
                    let float = (r.numerator as f32) / (r.denominator as f32);
                    write!(f, "{float}")
                }
                atom::Content::Boolean(b) => write!(f, "{b}"),
            }
        } else {
            write!(f, "(")?;
            write!(f, "{}", self.list.iter().format(" "))?;
            write!(f, ")")
        }
    }
}

#[derive(Clone, Debug)]
pub struct Assignment {
    pub fluent: Option<Expression>,
    pub value: Option<Expression>,
}

#[derive(Clone, Debug)]
pub struct Fluent {
    pub name: String,
    pub default_value: Option<Expression>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MetricKind {
    MinimizeActionCosts,
    MinimizeSequentialPlanLength,
    MinimizeMakespan,
    MinimizeExpressionOnFinalState,
    MaximizeExpressionOnFinalState,
    Oversubscription,
    TemporalOversubscription,
}

#[derive(Clone, Debug)]
pub struct Metric {
    pub kind: MetricKind,
    pub expression: Option<Expression>,
    pub action_costs: HashMap<String, Expression>,
    pub default_action_cost: Option<Expression>,
}

/// Reads the `name` attribute of a Python object.
pub fn name(obj: &Bound<PyAny>) -> Result<String> {
    Ok(obj.getattr("name")?.extract()?)
}

/// Returns true if the boolean method `method` of `obj` returns true.
pub fn is(obj: &Bound<PyAny>, method: &str) -> Result<bool> {
    Ok(obj.call_method0(method)?.is_truthy()?)
}

/// Collects the elements of a Python iterable.
pub fn list<'py>(obj: &Bound<'py, PyAny>) -> Result<Vec<Bound<'py, PyAny>>> {
    Ok(obj.try_iter()?.collect::<PyResult<Vec<_>>>()?)
}

/// Collects the `(key, value)` pairs of a Python dict, flattening the lists of values.
pub fn dict_of_lists<'py>(obj: &Bound<'py, PyAny>) -> Result<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
    let mut res = Vec::new();
    for item in list(&obj.call_method0("items")?)? {
        let (key, values): (Bound<PyAny>, Bound<PyAny>) = item.extract()?;
        for value in list(&values)? {
            res.push((key.clone(), value));
        }
    }
    Ok(res)
}

/// Returns true if the problem kind has the given feature (e.g. `"SCHEDULING"`).
pub fn has_feature(problem: &Bound<PyAny>, feature: &str) -> Result<bool> {
    Ok(problem.getattr("kind")?.getattr("features")?.contains(feature)?)
}

/// Name of a type, as used in the protobuf representation.
pub fn proto_type(tpe: &Bound<PyAny>) -> Result<String> {
    if is(tpe, "is_bool_type")? {
        Ok("up:bool".to_string())
    } else if is(tpe, "is_time_type")? {
        Ok("up:time".to_string())
    } else if is(tpe, "is_int_type")? || is(tpe, "is_real_type")? {
        Ok(format!("up:{}", tpe.str()?))
    } else if is(tpe, "is_user_type")? {
        name(tpe)
    } else {
        bail!("Unsupported type: {tpe}")
    }
}

fn symbol_expression(symbol: impl Into<String>, kind: ExpressionKind, tpe: impl Into<String>) -> Expression {
    Expression {
        atom: Some(Atom {
            content: Some(atom::Content::Symbol(symbol.into())),
        }),
        list: vec![],
        r#type: tpe.into(),
        kind,
    }
}

fn constant_expression(content: atom::Content, tpe: &str) -> Expression {
    Expression {
        atom: Some(Atom { content: Some(content) }),
        list: vec![],
        r#type: tpe.to_string(),
        kind: ExpressionKind::Constant,
    }
}

fn application(list: Vec<Expression>, tpe: impl Into<String>) -> Expression {
    Expression {
        atom: None,
        list,
        r#type: tpe.into(),
        kind: ExpressionKind::FunctionApplication,
    }
}

/// Converts a Python number (int or Fraction) into an expression.
pub fn num_expression(value: &Bound<PyAny>) -> Result<Expression> {
    if let Ok(i) = value.extract::<i64>() {
        Ok(constant_expression(atom::Content::Int(i), "up:integer"))
    } else {
        let r: Ratio<i64> = value.extract()?;
        Ok(constant_expression(atom::Content::Real(r.into()), "up:real"))
    }
}

/// Converts a UP parameter into a parameter expression.
pub fn parameter_expression(param: &Bound<PyAny>) -> Result<Expression> {
    Ok(symbol_expression(
        name(param)?,
        ExpressionKind::Parameter,
        proto_type(&param.getattr("type")?)?,
    ))
}

fn map_operator(op: &str) -> Result<&'static str> {
    Ok(match op {
        "PLUS" => "up:plus",
        "MINUS" => "up:minus",
        "TIMES" => "up:times",
        "DIV" => "up:div",
        "LE" => "up:le",
        "LT" => "up:lt",
        "EQUALS" => "up:equals",
        "AND" => "up:and",
        "OR" => "up:or",
        "NOT" => "up:not",
        "IMPLIES" => "up:implies",
        "IFF" => "up:iff",
        "EXISTS" => "up:exists",
        "FORALL" => "up:forall",
        "ALWAYS" => "up:always",
        "AT_MOST_ONCE" => "up:at_most_once",
        "SOMETIME" => "up:sometime",
        "SOMETIME_AFTER" => "up:sometime_after",
        "SOMETIME_BEFORE" => "up:sometime_before",
        _ => bail!("Unknown operator `{op}`"),
    })
}

/// Converts a UP `FNode` into an expression, mirroring `FNode2Protobuf` of the unified-planning library.
pub fn read_expression(e: &Bound<PyAny>) -> Result<Expression> {
    use ExpressionKind::*;
    let op: String = e.getattr("node_type")?.getattr("name")?.extract()?;
    let expr = match op.as_str() {
        "BOOL_CONSTANT" => constant_expression(
            atom::Content::Boolean(e.call_method0("bool_constant_value")?.extract()?),
            "up:bool",
        ),
        "INT_CONSTANT" | "REAL_CONSTANT" => num_expression(&e.call_method0("constant_value")?)?,
        "PARAM_EXP" => parameter_expression(&e.call_method0("parameter")?)?,
        "VARIABLE_EXP" => {
            let var = e.call_method0("variable")?;
            symbol_expression(name(&var)?, Variable, proto_type(&var.getattr("type")?)?)
        }
        "OBJECT_EXP" => {
            let obj = e.call_method0("object")?;
            symbol_expression(name(&obj)?, Constant, proto_type(&obj.getattr("type")?)?)
        }
        "PRESENT_EXP" => {
            let container: String = e.call_method0("presence")?.getattr("container")?.extract()?;
            application(
                vec![
                    symbol_expression("up:present", FunctionSymbol, ""),
                    symbol_expression(container, ContainerId, "up:container"),
                ],
                proto_type(&e.getattr("type")?)?,
            )
        }
        "TIMING_EXP" => {
            let timing = e.call_method0("timing")?;
            let tp = timing.getattr("timepoint")?;
            let fn_name = match tp.getattr("kind")?.getattr("name")?.extract::<String>()?.as_str() {
                "GLOBAL_START" => "up:global_start",
                "GLOBAL_END" => "up:global_end",
                "START" => "up:start",
                "END" => "up:end",
                kind => bail!("Unknown timepoint kind: {kind}"),
            };
            let mut list = vec![symbol_expression(fn_name, FunctionSymbol, "")];
            let container = tp.getattr("container")?;
            if !container.is_none() {
                list.push(symbol_expression(
                    container.extract::<String>()?,
                    ContainerId,
                    "up:container",
                ));
            }
            let tp_exp = application(list, "up:time");
            let delay = timing.getattr("delay")?;
            if delay.eq(0)? {
                // no delay for this timing, the expression is of the form (QUALIFIER [CONTAINER])
                tp_exp
            } else {
                // we have a delay, build an expression of the form (up:plus (QUALIFIER [CONTAINER]) DELAY)
                let add_exp = symbol_expression("up:plus", FunctionSymbol, "");
                application(vec![add_exp, tp_exp, num_expression(&delay)?], "up:time")
            }
        }
        "FLUENT_EXP" => {
            let fluent = e.call_method0("fluent")?;
            let tpe = proto_type(&fluent.getattr("type")?)?;
            let mut list = vec![symbol_expression(name(&fluent)?, FluentSymbol, tpe.clone())];
            for arg in list_args(e)? {
                list.push(arg);
            }
            Expression {
                atom: None,
                list,
                r#type: tpe,
                kind: StateVariable,
            }
        }
        _ => {
            let mut items = vec![symbol_expression(map_operator(&op)?, FunctionSymbol, "up:operator")];
            // forall/exists: add the declared variables to the beginning of the parameter list.
            if op == "EXISTS" || op == "FORALL" {
                for var in list(&e.call_method0("variables")?)? {
                    items.push(symbol_expression(
                        name(&var)?,
                        Variable,
                        proto_type(&var.getattr("type")?)?,
                    ));
                }
            }
            items.extend(list_args(e)?);
            application(items, "")
        }
    };
    Ok(expr)
}

fn list_args(e: &Bound<PyAny>) -> Result<Vec<Expression>> {
    list(&e.getattr("args")?)?.iter().map(read_expression).collect()
}

/// Converts the explicit initial assignments of a problem.
pub fn read_assignments(values: &Bound<PyAny>) -> Result<Vec<Assignment>> {
    let mut res = Vec::new();
    for item in list(&values.call_method0("items")?)? {
        let (fluent, value): (Bound<PyAny>, Bound<PyAny>) = item.extract()?;
        res.push(Assignment {
            fluent: Some(read_expression(&fluent)?),
            value: Some(read_expression(&value)?),
        });
    }
    Ok(res)
}

/// Reads the fluents of a problem, together with their default values.
pub fn read_fluents(problem: &Bound<PyAny>) -> Result<Vec<Fluent>> {
    let defaults = problem.getattr("fluents_defaults")?;
    let mut res = Vec::new();
    for fluent in list(&problem.getattr("fluents")?)? {
        let default = defaults.call_method1("get", (&fluent,))?;
        res.push(Fluent {
            name: name(&fluent)?,
            default_value: if default.is_none() {
                None
            } else {
                Some(read_expression(&default)?)
            },
        });
    }
    Ok(res)
}

/// Reads the quality metrics of a problem.
pub fn read_metrics(problem: &Bound<PyAny>) -> Result<Vec<Metric>> {
    let mut res = Vec::new();
    for metric in list(&problem.getattr("quality_metrics")?)? {
        let class: String = metric.get_type().name()?.extract()?;
        let kind = match class.as_str() {
            "MinimizeActionCosts" => MetricKind::MinimizeActionCosts,
            "MinimizeSequentialPlanLength" => MetricKind::MinimizeSequentialPlanLength,
            "MinimizeMakespan" => MetricKind::MinimizeMakespan,
            "MinimizeExpressionOnFinalState" => MetricKind::MinimizeExpressionOnFinalState,
            "MaximizeExpressionOnFinalState" => MetricKind::MaximizeExpressionOnFinalState,
            "Oversubscription" => MetricKind::Oversubscription,
            "TemporalOversubscription" => MetricKind::TemporalOversubscription,
            _ => bail!("Unsupported metric: {class}"),
        };
        let mut metric_data = Metric {
            kind,
            expression: None,
            action_costs: HashMap::new(),
            default_action_cost: None,
        };
        match kind {
            MetricKind::MinimizeActionCosts => {
                for item in list(&metric.getattr("costs")?.call_method0("items")?)? {
                    let (action, cost): (Bound<PyAny>, Bound<PyAny>) = item.extract()?;
                    metric_data.action_costs.insert(name(&action)?, read_expression(&cost)?);
                }
                let default = metric.getattr("default")?;
                if !default.is_none() {
                    metric_data.default_action_cost = Some(read_expression(&default)?);
                }
            }
            MetricKind::MinimizeExpressionOnFinalState | MetricKind::MaximizeExpressionOnFinalState => {
                metric_data.expression = Some(read_expression(&metric.getattr("expression")?)?);
            }
            _ => {}
        }
        res.push(metric_data);
    }
    Ok(res)
}
