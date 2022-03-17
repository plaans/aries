use anyhow::{anyhow, bail, Context, Error, Ok};
use aries_core::{Lit, INT_CST_MAX};
use aries_grpc_api::{Action, Expression, ExpressionKind, Problem};
use aries_model::extensions::Shaped;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_utils::input::Sym;
use std::collections::HashSet;
use std::sync::Arc;

/// Names for built in types. They contain UTF-8 symbols for sexiness
/// (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static FLUENT_TYPE: &str = "★fluent★";
static OBJECT_TYPE: &str = "★object★";

pub fn problem_to_chronicles(problem: &Problem) -> Result<aries_planning::chronicles::Problem, Error> {
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
    {
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
    }

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(types.clone(), symbols)?;

    let from_upf_type = |name: &str| {
        // TODO: Add in the upper and lower bound for int types using regex
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
    {
        for fluent in &problem.fluents {
            let sym = symbol_table
                .id(&Sym::from(fluent.name.clone()))
                .with_context(|| format!("Fluent `{}` not found in symbol table", fluent.name))?;
            let mut args = Vec::with_capacity(1 + fluent.parameters.len());

            for arg in &fluent.parameters {
                args.push(
                    from_upf_type(arg.name.as_str()).with_context(|| {
                        format!("Invalid parameter type `{}` for fluent `{}`", arg.name, fluent.name)
                    })?,
                );
            }

            args.push(from_upf_type(&fluent.value_type).with_context(|| {
                format!(
                    "Invalid return type `{}` for fluent `{}`",
                    fluent.value_type, fluent.name
                )
            })?);

            state_variables.push(StateFun { sym, tpe: args });
        }
    }

    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

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
            .fluent
            .as_ref()
            .context("Initial state assignment has no valid fluent")?;
        let value = init_state
            .value
            .as_ref()
            .context("Initial state assignment has no valid value")?;

        let (expr, _) = read_expression(expr, &context)?;
        let (_, value) = read_expression(value, &context)?;

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
        // TODO: Add temporal behaviour
        let goal_expr = goal.goal.as_ref().context("Goal has no valid expression")?;
        let (state_var, value) = read_expression(goal_expr, &context)?;

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
        let template = read_chronicle_template(cont, ChronicleAs::Action(a), &mut context)?;
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
fn read_sv(expr: &Expression, context: &Ctx) -> Result<Sv, Error> {
    let mut sv = Vec::new();
    if let Some(atom) = &expr.atom {
        let atom = read_atom(atom, context.model.get_symbol_table())?;
        // TODO: Read the values for the fluents
        match atom {
            Atom::Sym(s) => sv.push(s),
            _ => return Err(anyhow!("Unexpected atom type within expression")),
        }
    } else {
        // Parse the subexpression list
        for e in &expr.list {
            let _sv = read_sv(e, context)?;
            sv.extend(_sv);
        }
    }
    Ok(sv)
}

/// Transform a condition into an pair (state_variable, value) representing an equality condition between the two.
fn read_expression(expr: &Expression, context: &Ctx) -> Result<(Sv, Atom), Error> {
    let expr_kind =
        ExpressionKind::from_i32(expr.kind).with_context(|| format!("Unknown expression kind {}", expr.kind))?;

    // TODO: Problem in translating value for the SV
    match expr_kind {
        ExpressionKind::Unknown | ExpressionKind::Parameter => {
            bail!("Expected equality condition, but found unknown expression")
        }
        ExpressionKind::Constant | ExpressionKind::FluentSymbol => {
            let sv = read_sv(expr, context)?;
            let value = expr.atom.as_ref().unwrap().clone();
            let value = read_atom(&value, context.model.get_symbol_table())?;
            Ok((sv, value))
        }
        ExpressionKind::FunctionSymbol | ExpressionKind::FunctionApplication => {
            if !is_op("=", expr) {
                bail!("Expected equality condition");
            }
            let sv = read_sv(expr, context)?;
            let value = expr.atom.as_ref().unwrap().clone();
            let value =
                read_atom(&value, context.model.get_symbol_table()).with_context(|| "Expected constant".to_string())?;
            Ok((sv, value))
        }
        ExpressionKind::StateVariable => {
            let sv = read_sv(expr, context)?;
            let value = expr.atom.as_ref().unwrap().clone();
            let value = read_atom(&value, context.model.get_symbol_table())?;
            Ok((sv, value))
        }
    }
}

fn read_condition(cond: &aries_grpc_api::Condition, context: &Ctx) -> Result<(Sv, Atom), Error> {
    if let Some(_span) = &cond.span {
        // TODO: Implement the durative condition
        unimplemented!()
    } else {
        let cond = cond.cond.as_ref().context("Condition has no valid expression")?;
        let (sv, value) = read_expression(cond, context)?;
        Ok((sv, value))
    }
}

fn read_effect(eff: &aries_grpc_api::Effect, context: &Ctx) -> Result<(Sv, Atom), Error> {
    if let Some(_occurence_time) = &eff.occurence_time {
        // TODO: Implement the durative effect
        unimplemented!()
    } else {
        let effect = eff.effect.as_ref().unwrap();
        let expr = effect
            .fluent
            .as_ref()
            .with_context(|| "Expected fluent expression".to_string())?;
        let value = effect
            .value
            .as_ref()
            .with_context(|| "Expected value expression".to_string())?;

        let (sv, _) = read_expression(expr, context)?;
        let (_, value) = read_expression(value, context)?;

        Ok((sv, value))
    }
}

fn is_op(operator: &str, expr: &Expression) -> bool {
    if let Some(atom) = &expr.atom {
        return match &atom.content {
            Some(aries_grpc_api::atom::Content::Symbol(s)) => s == operator,
            _ => false,
        };
    };
    false
}

fn read_atom(atom: &aries_grpc_api::Atom, symbol_table: &SymbolTable) -> Result<aries_model::lang::Atom, Error> {
    if let Some(atom_content) = atom.content.clone() {
        match atom_content {
            aries_grpc_api::atom::Content::Symbol(s) => {
                let atom = str_to_symbol(s.as_str(), symbol_table)?;
                Ok(Atom::from(atom))
            }
            aries_grpc_api::atom::Content::Int(i) => Ok(Atom::from(i)),
            aries_grpc_api::atom::Content::Float(_f) => {
                bail!("Float not supported yet")
            }
            aries_grpc_api::atom::Content::Boolean(b) => Ok(Atom::Bool(b.into())),
        }
    } else {
        Err(anyhow!("Unsupported type for atom"))
    }
}

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

fn read_chronicle_template(c: Container, action: ChronicleAs, context: &mut Ctx) -> Result<ChronicleTemplate, Error> {
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
    for param in &action.parameters {
        let arg = Sym::from(param.name.clone());
        let arg_type = Sym::from(param.r#type.clone());
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
        let eff = read_effect(eff, context)
            .map_err(|e| e.context(anyhow!("Action `{}` has an invalid effect", &action.name)))?;
        ch.effects.push(Effect {
            transition_start: start,
            persistence_start: end,
            state_var: eff.0,
            value: eff.1,
        });
    }

    let positive_effects: HashSet<Sv> = ch
        .effects
        .iter()
        .filter(|e| e.value == Atom::from(true))
        .map(|e| e.state_var.clone())
        .collect();
    ch.effects
        .retain(|e| e.value != Atom::from(false) || !positive_effects.contains(&e.state_var));

    for condition in &action.conditions {
        let condition = read_condition(condition, context)
            .map_err(|e| e.context(anyhow!("Action `{}` has an invalid condition", &action.name)))?;
        ch.conditions.push(Condition {
            start,
            end,
            state_var: condition.0,
            value: condition.1,
        })
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
