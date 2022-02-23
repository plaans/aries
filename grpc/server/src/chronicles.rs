use anyhow::{anyhow, bail, ensure, Context, Error};
use aries_grpc_api::{Action, Assignment, Expression, Problem};
use aries_model::bounds::Lit;
use aries_model::extensions::Shaped;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_utils::input::Sym;
use regex::Regex;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::sync::Arc;

/// Names for built in types. They contain UTF-8 symbols for sexiness (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static FLUENT_TYPE: &str = "★fluent★";
static OBJECT_TYPE: &str = "★object★";

pub fn problem_to_chronicles(problem: Problem) -> Result<aries_planning::chronicles::Problem, Error> {
    // Construct the type hierarchy
    let types = {
        // Static types present in any problem
        let mut types: Vec<(Sym, Option<Sym>)> = vec![
            (TASK_TYPE.into(), None),
            (ABSTRACT_TASK_TYPE.into(), Some(TASK_TYPE.into())),
            (ACTION_TYPE.into(), Some(TASK_TYPE.into())),
            (DURATIVE_ACTION_TYPE.into(), Some(TASK_TYPE.into())),
            (METHOD_TYPE.into(), None),
            (FLUENT_TYPE.into(), None),
            (OBJECT_TYPE.into(), None),
        ];

        // Object types are currently not explicitly declared in the model.
        // Extract all types used in objects declared and add them.
        for obj in &problem.objects {
            let object_type = Sym::from(obj.r#type.clone());

            //check if type is already in types
            if !types.iter().any(|(t, _)| t == &object_type) {
                types.push((object_type.clone(), Some(OBJECT_TYPE.into())));
            }
        }
        // we have all the types, build the hierarchy
        TypeHierarchy::new(types)?
    };

    // determine the top types in the user-defined hierarchy.
    // this is typically "object" by convention but might something else (e.g. "obj" in some hddl problems).
    let mut symbols: Vec<TypedSymbol> = vec![];

    // Types are currently not explicitly declared
    for obj in &problem.objects {
        let object_symbol = Sym::from(obj.name.clone());
        let object_type = Sym::from(obj.r#type.clone());

        // declare the object as a new symbol with the given type
        symbols.push(TypedSymbol {
            symbol: object_symbol.clone(),
            tpe: Some(object_type),
        });
    }

    // record all symbols representing fluents
    for fluent in &problem.fluents {
        symbols.push(TypedSymbol {
            symbol: Sym::from(fluent.name.clone()),
            tpe: Some(FLUENT_TYPE.into()),
        });
    }

    // actions are symbols as well, add them to the table
    for action in &problem.actions {
        symbols.push(TypedSymbol {
            symbol: Sym::from(action.name.clone()),
            tpe: Some(ACTION_TYPE.into()),
        });
    }

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(types.clone(), symbols)?;

    let from_upf_type = |name: &str| {
        if name == "bool" {
            Ok(Type::Bool)
        } else if name == "int" {
            Ok(Type::Int)
        } else if let Some(tpe) = types.id_of(name) {
            Ok(Type::Sym(tpe))
        } else {
            Err(anyhow!("Unsupported type `{}`", name))
        }
    };

    let mut state_variables = vec![];
    for fluent in &problem.fluents {
        let sym = symbol_table
            .id(&Sym::from(fluent.name.clone()))
            .with_context(|| format!("Fluent {} not found in symbol table", fluent.name))?;
        let mut args = Vec::with_capacity(1 + fluent.signature.len());

        for arg in &fluent.signature {
            args.push(
                from_upf_type(arg.as_str())
                    .with_context(|| format!("Invalid parameter type for fluent `{}`", fluent.name))?,
            );
        }

        args.push(
            from_upf_type(&fluent.value_type)
                .with_context(|| format!("Invalid return type for fluent `{}`", fluent.name))?,
        );

        state_variables.push(StateFun { sym, tpe: args });
    }

    let mut context = Ctx::new(Arc::new(symbol_table.clone()), state_variables);

    // Initial chronicle construction
    let mut init_ch = Chronicle {
        kind: ChronicleKind::Problem,
        presence: Lit::TRUE,
        start: context.origin(),
        end: context.horizon(),
        name: vec![],
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Initial state translates as effect at the global start time
    for init_state in &problem.initial_state {
        let expr = init_state
            .x
            .as_ref()
            .context("Initial state assignment has no valid fluent")?;
        let value = init_state
            .v
            .as_ref()
            .context("Initial state assignment has no valid value")?;

        let expr = read_sv(expr, &problem, &symbol_table, &read_constant_atom)?;
        let value = read_constant_atom(value, &symbol_table)?;

        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var: expr,
            value,
        })
    }

    // goals translate as condition at the global end time
    for goal in &problem.goals {
        // a goal is simply a condition where only constant atom can appear
        let (state_var, value) = read_condition(goal, &problem, &symbol_table, &read_constant_atom)?;

        init_ch.conditions.push(Condition {
            start: init_ch.end,
            end: init_ch.end,
            state_var,
            value,
        })
    }

    // TODO: Task networks?
    println!("=====");
    dbg!(&init_ch);
    println!("=====");

    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    for a in &problem.actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, &problem, ChronicleAs::Action(a), &mut context)?;
        templates.push(template);
    }

    //TODO: Add methods and durative actions to the templates

    let problem = aries_planning::chronicles::Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    Ok(problem)
}

fn str_to_symbol(name: &str, symbol_table: &SymbolTable) -> anyhow::Result<SAtom> {
    let sym = symbol_table
        .id(name)
        .with_context(|| format!("Unknown symbol / operator `{}`", name))?;
    let tpe = symbol_table.type_of(sym);
    Ok(SAtom::new_constant(sym, tpe))
}

/// Transforms an expression into a state variable (returning an error if it is not a state variable)
///
/// The function expect a `read_atom` function that is used to transform an Expression into an atom.
/// This is necessary because this translation is context-dependent:
///  - in the initial facts or goals, an atom is simply a constant (symbol, symbol)
///  - inside an action, a string might refer to an action parameter.
///    In this case `read_atom` should return the corresponding variable that was created to represent the parameter (wrapped into an `Atom`)
fn read_sv(
    expr: &Expression,
    problem: &Problem,
    symbol_table: &SymbolTable,
    read_atom: &impl Fn(&Expression, &SymbolTable) -> anyhow::Result<Atom>,
) -> Result<Sv, Error> {
    let mut sv = Vec::new();

    let head_name = expr.payload.as_ref().context("missing payload")?.value.as_str();
    let head = str_to_symbol(head_name, symbol_table)?;
    // TODO: ensure that this requires the full Context object (and not only the Symbol table

    let fluent = problem
        .fluents
        .iter()
        .find(|fluent| fluent.name == head_name)
        .ok_or_else(|| anyhow!("Unknown fluent `{}`", head_name))?;

    //  - args has the same number of arguments as the declared fluent
    if fluent.signature.len() != expr.args.len() {
        return Err(anyhow!(
            "Fluent `{}` has {} arguments, but {} were provided",
            fluent.name,
            fluent.signature.len(),
            expr.args.len()
        ));
    }
    sv.push(head);

    for arg in &expr.args {
        let atom = read_atom(arg, symbol_table)?;
        let satom = SAtom::try_from(atom)?;
        sv.push(satom);
    }
    Ok(sv)
}

/// Transform a condition into an pair (state_variable, value) representing an equality condition between the two.
///
/// See the doc of `read_sv` for the usage of the `read_atom` parameter.
fn read_condition(
    expr: &Expression,
    problem: &Problem,
    symbol_table: &SymbolTable,
    read_atom: &impl Fn(&Expression, &SymbolTable) -> anyhow::Result<Atom>,
) -> Result<(Sv, Atom), Error> {
    let sv = read_sv(expr, problem, symbol_table, read_atom);
    if let Ok(sv) = sv {
        // the expression is a single fluent. E.g. (at-robot l1)
        let fluent_name = expr
            .payload
            .as_ref()
            .context("Missing Payload in conditions")?
            .value
            .as_str();
        let fluent = problem
            .fluents
            .iter()
            .find(|fluent| fluent.name == fluent_name)
            .ok_or_else(|| anyhow!("Unknown fluent in action conditions `{}`", fluent_name))?;

        if fluent.value_type != "bool" {
            return Err(anyhow!("Fluent `{}` is not of boolean type", fluent.name));
        }
        // return (at-robot l1) = true
        Ok((sv, Atom::from(true)))
    } else if is_op("=", expr) {
        // has form (= (at-robot l1) true)
        ensure!(expr.args.len() == 2, "Equality has the wrong number of args");
        // left-hand side must be a fluent
        let sv = read_sv(&expr.args[0], problem, symbol_table, read_atom)?;
        // right hand side must be a constant
        let value = read_constant_atom(&expr.args[1], symbol_table)?;
        Ok((sv, value))
    } else {
        Err(sv.unwrap_err())
    }
}

fn read_effect(
    eff: &Assignment,
    problem: &Problem,
    symbol_table: &SymbolTable,
    read_atom: &impl Fn(&Expression, &SymbolTable) -> anyhow::Result<Atom>,
) -> Result<(Sv, Atom), Error> {
    let expr = &eff.x.as_ref().unwrap();
    let val = &eff.v.as_ref().unwrap();

    let sv = read_sv(expr, problem, symbol_table, read_atom);
    if let Ok(sv) = sv {
        // the expression is a single fluent. E.g. (at-robot l1)
        let fluent_name = expr
            .payload
            .as_ref()
            .context("Missing Payload in effect expressions")?
            .value
            .as_str();
        let fluent = problem
            .fluents
            .iter()
            .find(|fluent| fluent.name == fluent_name)
            .ok_or_else(|| anyhow!("Unknown fluent in effect `{}`", fluent_name))?;

        if fluent.value_type != "bool" {
            return Err(anyhow!("Fluent `{}` is not of boolean type", fluent.name));
        }
        let value = read_constant_atom(val, symbol_table)?;
        Ok((sv, value))
    } else if is_op("=", expr) {
        // has form (= (at-robot l1) true)
        ensure!(expr.args.len() == 2, "Equality has the wrong number of args");
        // left-hand side must be a fluent
        let sv = read_sv(&expr.args[0], problem, symbol_table, read_atom)?;
        // right hand side must be a constant
        let value = read_constant_atom(&expr.args[1], symbol_table)?;
        Ok((sv, value))
    } else {
        Err(sv.unwrap_err())
    }
}

fn is_op(operator: &str, expr: &Expression) -> bool {
    expr.payload.as_ref().map(|pl| pl.value == operator).unwrap_or(false)
}

fn read_constant_atom(expr: &Expression, symbol_table: &SymbolTable) -> Result<Atom, Error> {
    ensure!(expr.args.is_empty(), "Expected atom but got an expression");
    let payload = expr.payload.as_ref().context("Empty payload")?;

    let mut tpe = payload.r#type.as_str();

    // RegEx to extract the type `<type>[<lower_bound>..<upper_bound>]`
    let re = Regex::new(r"^(?P<type>[a-zA-Z0-9_]+)(\[(?P<lower_bound>[0-9]+)\..*\])?$").unwrap();
    let caps = re.captures(tpe).unwrap();
    tpe = caps.name("type").unwrap().as_str();
    let lb = caps.name("lower_bound").map(|s| s.as_str().parse::<i32>().unwrap());
    let ub = caps.name("upper_bound").map(|s| s.as_str().parse::<i32>().unwrap());

    match tpe {
        "bool" => match payload.value.as_str() {
            "True" => Ok(Lit::TRUE.into()),
            "False" => Ok(Lit::FALSE.into()),
            x => bail!("Unknown boolean constant `{}`", x),
        },
        "int" => {
            let i = payload.value.parse::<i32>().context("Expected integer")?;
            if let Some(lb) = lb {
                ensure!(i >= lb, "Integer value {} is smaller than lower bound {}", i, lb);
            }
            if let Some(ub) = ub {
                ensure!(i <= ub, "Integer value {} is greater than upper bound {}", i, ub);
            }
            Ok(i.into())
        }
        "real" => {
            bail!("Real constants are not supported yet");
        }
        _ => Ok(str_to_symbol(&payload.value, symbol_table)?.into()),
    }
}

//
// TODO: Replace Action_ with Enum of Action, Method, and DurativeAction
pub enum ChronicleAs<'a> {
    Action(&'a Action),
    // Method(&'a Method),
    // DurativeAction(&'a DurativeAction),
}

impl ChronicleAs<'_> {
    fn kind(&self) -> ChronicleKind {
        match self {
            ChronicleAs::Action(_action) => ChronicleKind::Action,
            // ChronicleAs::Method(method) => ChronicleKind::Method,
            // ChronicleAs::DurativeAction(durative_action) => ChronicleKind::DurativeAction,
        }
    }
}

fn read_chronicle_template(
    c: Container,
    problem: &Problem,
    action: ChronicleAs,
    context: &mut Ctx,
) -> Result<ChronicleTemplate, Error> {
    let action_kind = action.kind();
    let ChronicleAs::Action(action) = action;

    let mut params: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = match action_kind {
        ChronicleKind::Problem => bail!("Problem type not supported"),
        ChronicleKind::Method | ChronicleKind::DurativeAction => {
            let end = context
                .model
                .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
            params.push(end.into());
            end.into()
        }
        ChronicleKind::Action => start + FAtom::EPSILON,
    };

    let mut name: Vec<SAtom> = Vec::with_capacity(1 + action.parameters.len());
    let base_name = &Sym::from(action.name.clone());
    name.push(
        context
            .typed_sym(
                context
                    .model
                    .get_symbol_table()
                    .id(base_name)
                    .ok_or_else(|| base_name.invalid("Unknown action"))?,
            )
            .into(),
    );

    // Process, the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for (arg, arg_type) in action
        .parameters
        .clone()
        .into_iter()
        .zip(action.parameter_types.clone())
    {
        let arg = Sym::from(arg.clone());
        let arg_type = Sym::from(arg_type.clone());
        let tpe = context
            .model
            .get_symbol_table()
            .types
            .id_of(&arg_type)
            .ok_or_else(|| arg.invalid("Unknown argument"))?;
        let arg = context.model.new_optional_sym_var(tpe, prez, c / VarType::Parameter); // arg.symbol
        params.push(arg.into());
        name.push(arg.into());
    }

    let symbol_table = context.model.get_symbol_table();

    // in the context of an action, an Atom can be either a constant, or a variable (i.e. a parameter of the action)
    let read_atom = |expr: &Expression, symbol_table: &SymbolTable| -> anyhow::Result<Atom> {
        ensure!(expr.args.is_empty(), "Not an atom");
        let id = expr.payload.as_ref().context("no payload")?.value.as_str();
        match action.parameters.iter().position(|param| param == id) {
            Some(i) => {
                // this is a param, return the corresponding variable that we created for it
                // first element of the name is the base_name, others are the parameters
                Ok(name[i + 1].into())
            }
            None => {
                // not an action parameter, must be a constant atom
                read_constant_atom(expr, symbol_table)
            }
        }
    };

    let mut ch = Chronicle {
        kind: action_kind,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Process the effects of the action
    for eff in &action.effects {
        let eff = read_effect(eff, problem, symbol_table, &read_atom);
        // let state_var = read_sv(eff.x.as_ref().context("no sv")?, &problem, symbol_table, &read_atom)?;
        // let value = read_atom(eff.v.as_ref().context("no value")?, symbol_table)?;
        if let Ok((state_var, value)) = eff {
            ch.effects.push(Effect {
                transition_start: start,
                persistence_start: end,
                state_var,
                value,
            });
        } else {
            return Err(anyhow!(
                "Action `{}` has an invalid effect. Throws: {}",
                action.name,
                eff.unwrap_err()
            ));
        }
    }

    let positive_effects: HashSet<Sv> = ch
        .effects
        .iter()
        .filter(|e| e.value == Atom::from(true))
        .map(|e| e.state_var.clone())
        .collect();
    ch.effects
        .retain(|e| e.value != Atom::from(false) || !positive_effects.contains(&e.state_var));

    for condition in &action.preconditions {
        let condition = read_condition(condition, problem, symbol_table, &read_atom);
        if let Ok((state_var, value)) = condition {
            ch.conditions.push(Condition {
                start,
                end,
                state_var,
                value,
            })
        } else {
            return Err(anyhow!(
                "Action `{}` has an invalid condition. Throws: {}",
                action.name,
                condition.unwrap_err()
            ));
        }
    }
    println!("===");
    dbg!(&ch);
    println!("===");

    Ok(ChronicleTemplate {
        label: Some(action.name.clone()),
        parameters: params,
        chronicle: ch,
    })
}
