use anyhow::{anyhow, bail, Context, Error, Ok};
use aries_core::{Lit, INT_CST_MAX};
use aries_grpc_api::timepoint::TimepointKind;
use aries_grpc_api::{Expression, ExpressionKind, Problem};
use aries_model::extensions::Shaped;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_utils::input::Sym;
use std::collections::{HashMap, HashSet};
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
        if name == "bool" {
            Ok(Type::Bool)
        } else if name.starts_with("int") {
            // Can account for int[0,1] or integer or integer[0,1]
            Ok(Type::Int)
        } else if name.starts_with("real") {
            Err(anyhow!("Real types are not supported"))
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
                args.push(from_upf_type(arg.r#type.as_str()).with_context(|| {
                    format!("Invalid parameter type `{}` for fluent `{}`", arg.r#type, fluent.name)
                })?);
            }

            args.push(from_upf_type(&fluent.value_type)?);

            state_variables.push(StateFun { sym, tpe: args });
        }
    }

    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);
    println!("===== Symbol Table =====");
    println!("{:?}", context.model.get_symbol_table());

    println!("===== State Variables =====");
    println!("{:?}", context.state_functions);

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
    println!("===== Initial state =====");
    for init_state in &problem.initial_state {
        let expr = init_state
            .fluent
            .as_ref()
            .context("Initial state assignment has no valid fluent")?;
        let value = init_state
            .value
            .as_ref()
            .context("Initial state assignment has no valid value")?;

        let (state_var, value) = read_initial_state(expr, value, &context)?;

        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var,
            value,
        })
    }

    // goals translate as condition at the global end time
    println!("===== Goals =====");
    for goal in &problem.goals {
        // a goal is simply a condition where only constant atom can appear
        // TODO: Add temporal behaviour
        let goal_expr = goal.goal.as_ref().context("Goal has no valid expression")?;
        let (state_var, value) = read_goal_state(goal_expr, &context)?;

        init_ch.conditions.push(Condition {
            start: init_ch.end,
            end: init_ch.end,
            state_var,
            value,
        })
    }
    println!("===== Initial Chronicle =====");
    dbg!(&init_ch);

    // TODO: Task networks?
    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    for a in &problem.actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, a, &mut context)?;
        templates.push(template);
    }

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

//Read initial state with possible `Function Symbols` prefixed
fn read_initial_state(expr: &Expression, value: &Expression, context: &Ctx) -> Result<(Sv, Atom), Error> {
    let expr = read_state_variable(expr, context, &None)?;
    let value = read_value(value, context, &None)?;
    Ok((expr, value))
}

fn read_goal_state(goal_expr: &Expression, context: &Ctx) -> Result<(Sv, Atom), Error> {
    let expr = goal_expr.clone();
    let value = goal_expr.clone();
    let expr = read_state_variable(&expr, context, &None)?;
    let value = read_value(&value, context, &None)?;
    Ok((expr, value))
}

// read_atom` functions can return `Atom` or `SAtom`
enum AtomOrSAtom<S, T> {
    Atom(S),  // Atom
    SAtom(T), // SAtom
}

impl From<AtomOrSAtom<Atom, SAtom>> for SAtom {
    fn from(atom: AtomOrSAtom<Atom, SAtom>) -> SAtom {
        match atom {
            AtomOrSAtom::Atom(atom) => panic!("Expected SAtom, got Atom {:?}", atom),
            AtomOrSAtom::SAtom(satom) => satom,
        }
    }
}

impl From<AtomOrSAtom<Atom, SAtom>> for Atom {
    fn from(atom: AtomOrSAtom<Atom, SAtom>) -> Atom {
        match atom {
            AtomOrSAtom::Atom(atom) => atom,
            AtomOrSAtom::SAtom(satom) => Atom::from(satom),
        }
    }
}

fn read_atom(
    atom: &aries_grpc_api::Atom,
    symbol_table: &SymbolTable,
) -> Result<AtomOrSAtom<aries_model::lang::Atom, aries_model::lang::SAtom>, Error> {
    if let Some(atom_content) = atom.content.clone() {
        match atom_content {
            aries_grpc_api::atom::Content::Symbol(s) => {
                let atom = str_to_symbol(s.as_str(), symbol_table)?;
                Ok(AtomOrSAtom::SAtom(atom)) // Handles SAtom
            }
            aries_grpc_api::atom::Content::Int(i) => Ok(AtomOrSAtom::Atom(Atom::from(i))),
            aries_grpc_api::atom::Content::Real(_f) => {
                bail!("`Real` type not supported yet")
            }
            aries_grpc_api::atom::Content::Boolean(b) => Ok(AtomOrSAtom::Atom(Atom::Bool(b.into()))),
        }
    } else {
        Err(anyhow!("Unsupported atom"))
    }
}

/// Read the expression and return the state variables for the expressions
/// The expression parameters can be of type `FluentSymbol` or `StateVariable` or `Parameter`
/// The expression type `FunctionApplication` should hold the following in order:
/// - FunctionSymbol
/// - List of parameters (FluentSymbol or StateVariable or Parameter)
fn read_state_variable(
    expr: &Expression,
    context: &Ctx,
    _parameter_mapping: &Option<HashMap<String, Variable>>,
) -> Result<Sv, Error> {
    let mut sv = Vec::new();
    let expr_kind = ExpressionKind::from_i32(expr.kind).unwrap();

    if expr_kind == ExpressionKind::Constant {
        Ok(vec![read_atom(
            expr.atom.as_ref().unwrap(),
            context.model.get_symbol_table(),
        )?
        .into()])
    } else if expr_kind == ExpressionKind::Parameter {
        // No state variable for parameters
        Ok(vec![])
    } else if expr_kind == ExpressionKind::FunctionApplication {
        assert_eq!(expr.atom, None, "Function application should not have an atom");

        let mut sub_list = expr.list.clone();

        while let Some(sub_expr) = sub_list.pop() {
            let sub_expr_kind = ExpressionKind::from_i32(sub_expr.kind).unwrap();
            if sub_expr_kind == ExpressionKind::Constant {
                continue;
            } else if sub_expr_kind == ExpressionKind::FunctionSymbol {
                assert!(sub_expr.atom.is_some(), "Function symbol should have an atom");
                let operator = sub_expr.atom.as_ref().unwrap().content.as_ref().unwrap();
                if let aries_grpc_api::atom::Content::Symbol(operator) = operator.clone() {
                    match operator.as_str() {
                        "=" => {
                            let mut sub_list = sub_list.clone();
                            for sub_expr in sub_list.iter_mut() {
                                let state_var = read_state_variable(sub_expr, context, &None)?;
                                sv.extend(state_var);
                            }
                        }
                        "and" => {
                            todo!("`and` operator not supported yet")
                        }
                        "not" => {
                            todo!("`not` operator not supported yet")
                        }
                        _ => {
                            bail!("Unsupported operator `{}`", operator)
                        }
                    }
                } else {
                    bail!("Operator {:?} should be a symbol", operator);
                }
            } else {
                let state_var = read_state_variable(&sub_expr, context, &None)?;
                sv.extend(state_var);
            }
        }
        Ok(sv)
    } else if expr_kind == ExpressionKind::StateVariable {
        assert_eq!(expr.atom, None, "StateVariable should not have an atom");

        let mut sub_list = expr.list.clone();

        while let Some(sub_expr) = sub_list.pop() {
            if sub_expr.kind == ExpressionKind::FluentSymbol as i32 {
                match read_atom(sub_expr.atom.as_ref().unwrap(), context.model.get_symbol_table())? {
                    AtomOrSAtom::SAtom(fluent) => sv.push(fluent),
                    _ => bail!("Expected a valid fluent symbol as atom in expression"),
                }
            } else {
                let state_var = read_state_variable(&sub_expr, context, &None)?;
                sv.extend(state_var);
            }
        }
        // FIXME: this is a hack to make sure that the state variables are sorted
        sv.reverse();
        Ok(sv)
    } else {
        bail!(anyhow!("Unsupported expression kind: {:?}", expr_kind))
    }
}

/// Read the expression and return the values from the expression
/// THe expressions of type `Constant` or `FluentSymbol` or `StateVariable`
/// The expression type `FunctionApplication` should hold the following in order:
/// - FunctionSymbol
/// - List of parameters (Constant or FluentSymbol or StateVariable)
/// The supported value expressions are:
/// (= <fluent> <value>) // Function Application Type
/// (<fluent>) // State Variable Type
///
fn read_value(
    expr: &aries_grpc_api::Expression,
    context: &Ctx,
    parameter_mapping: &Option<HashMap<String, Variable>>,
) -> Result<Atom, Error> {
    let expr_kind = ExpressionKind::from_i32(expr.kind).unwrap();
    if expr_kind == ExpressionKind::Constant {
        Ok(read_atom(expr.atom.as_ref().unwrap(), context.model.get_symbol_table())?.into())
    } else if expr_kind == ExpressionKind::Parameter {
        assert!(expr.atom.is_some(), "Parameter should have an atom");

        // Parameteric values are variables
        let parameter_name = expr.atom.as_ref().unwrap().content.as_ref().unwrap();
        match parameter_name {
            aries_grpc_api::atom::Content::Symbol(s) => {
                let parameter_name = s.as_str();
                let var = parameter_mapping
                    .as_ref()
                    .unwrap()
                    .get(parameter_name)
                    .with_context(|| format!("Parameter `{:?}` not found", expr.atom))?;
                match var {
                    Variable::Int(i) => Ok(Atom::Int(IAtom::from(*i))),
                    Variable::Bool(b) => Ok(Atom::Bool(b.true_lit())),
                    Variable::Fixed(f) => Ok(Atom::Fixed(FAtom::from(*f))),
                    Variable::Sym(s) => Ok(Atom::from(SAtom::from(*s))),
                }
            }
            _ => bail!("Parameter should be a symbol"),
        }
    } else if expr_kind == ExpressionKind::StateVariable {
        assert_eq!(
            expr.atom, None,
            "Value Expression of type `StateVariable` should not have an atom"
        );
        let mut sub_list = expr.list.clone();
        let atom = read_atom(
            sub_list.pop().unwrap().atom.as_ref().unwrap(),
            context.model.get_symbol_table(),
        )?;
        Ok(atom.into())
    } else if expr_kind == ExpressionKind::FunctionApplication {
        assert_eq!(
            expr.atom, None,
            "Value Expression of type `StateVariable` should not have an atom"
        );

        let mut sub_list = expr.list.clone();
        // First element is going to be function symbol
        let expr_head = sub_list.pop().unwrap();
        if expr_head.kind == ExpressionKind::FunctionSymbol as i32 {
            let operator = expr_head.atom.as_ref().unwrap().content.as_ref().unwrap();
            if let aries_grpc_api::atom::Content::Symbol(operator) = operator.clone() {
                match operator.as_str() {
                    "=" => {
                        assert_eq!(sub_list.len(), 2, "`=` operator should have exactly 2 arguments");
                        let value = read_atom(
                            sub_list.last().unwrap().atom.as_ref().unwrap(),
                            context.model.get_symbol_table(),
                        )?;
                        Ok(value.into())
                    }
                    "and" => {
                        todo!("`and` operator not supported yet")
                    }
                    "not" => {
                        todo!("`not` operator not supported yet")
                    }
                    _ => {
                        bail!("Unsupported operator `{}`", operator)
                    }
                }
            } else {
                bail!("Operator {:?} should be a symbol", expr_head.atom);
            }
        } else if expr_head.kind == ExpressionKind::Constant as i32 {
            read_value(&expr_head, context, parameter_mapping)
        } else {
            bail!("Unsupported expression kind: {:?}", expr_head)
        }
    } else {
        bail!("Unsupported expression kind: {:?}", expr_kind)
    }
}

fn read_timing(timing: &aries_grpc_api::Timing, context: &mut Ctx) -> Result<FAtom, Error> {
    let timing = timing.clone();
    match TimepointKind::from_i32(timing.timepoint.unwrap().kind).unwrap() {
        TimepointKind::GlobalStart => Ok(context.origin()),
        TimepointKind::GlobalEnd => Ok(context.horizon()),
        TimepointKind::Start => {
            let _start_time = timing.delay;
            // let start_time = FAtom::from(start_time as f32);
            // Ok(start_time)
            todo!("Start time not supported yet")
        }
        TimepointKind::End => todo!(),
    }
}

fn read_time_interval(interval: &aries_grpc_api::TimeInterval, context: &mut Ctx) -> Result<(FAtom, FAtom), Error> {
    let interval = interval.clone();
    let start = read_timing(&interval.lower.unwrap(), context)?;
    let end = read_timing(&interval.upper.unwrap(), context)?;
    Ok((start, end))
}
fn read_condition(
    cond: &aries_grpc_api::Expression,
    context: &Ctx,
    parameter_mapping: &Option<HashMap<String, Variable>>,
) -> Result<(Sv, Atom), Error> {
    let sv = read_state_variable(cond, context, parameter_mapping)?;
    let value = read_value(cond, context, parameter_mapping)?;
    Ok((sv, value))
}

fn read_effect(
    eff: &aries_grpc_api::EffectExpression,
    context: &Ctx,
    parameter_mapping: &Option<HashMap<String, Variable>>,
) -> Result<(Sv, Atom), Error> {
    let expr = eff
        .fluent
        .as_ref()
        .with_context(|| "Expected a valid fluent expression".to_string())?;
    let value = eff
        .value
        .as_ref()
        .with_context(|| "Expected a valid value expression".to_string())?;

    let sv = read_state_variable(expr, context, parameter_mapping)?;
    let value = read_value(value, context, parameter_mapping)?;

    Ok((sv, value))
}

fn read_chronicle_template(
    c: Container,
    action: &aries_grpc_api::Action,
    context: &mut Ctx,
) -> Result<ChronicleTemplate, Error> {
    let action_kind = {
        if action.duration.is_some() {
            ChronicleKind::DurativeAction
        } else {
            ChronicleKind::Action
        }
    };
    let mut params: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();
    let mut parameter_mapping: HashMap<String, Variable> = HashMap::new();

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

        // Add parameters to the mapping
        parameter_mapping.insert(param.name.clone(), arg.into());
    }

    println!("{:?}", &parameter_mapping);

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
        let eff = eff.clone();
        let eff_experssion = eff.effect.as_ref().context("Effect has no valid expression")?;
        let result = read_effect(eff_experssion, context, &Some(parameter_mapping.clone()));
        if let Some(occurence) = &eff.occurence_time {
            let _occurence = read_timing(occurence, context)?;
            //TODO: Add occurence time to the chronicle
        } else {
            match result {
                Result::Ok((state_var, value)) => {
                    ch.effects.push(Effect {
                        transition_start: start,
                        persistence_start: end,
                        state_var,
                        value,
                    });
                }
                Result::Err(e) => {
                    return Err(anyhow!(
                        "Action {} has an invalid effect: {}",
                        action.name,
                        e.to_string()
                    ))
                }
            }
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

    for condition in &action.conditions {
        let condition = condition.clone();
        let result = read_condition(&condition.cond.unwrap(), context, &Some(parameter_mapping.clone()));
        if let Some(span) = condition.span {
            let _span = read_time_interval(&span, context)?;
        } else {
            match result {
                Result::Ok(condition) => ch.conditions.push(Condition {
                    start,
                    end,
                    state_var: condition.0,
                    value: condition.1,
                }),
                Result::Err(e) => {
                    return Err(anyhow!(
                        "Action {} has an invalid condition: {}",
                        action.name,
                        e.to_string()
                    ))
                }
            }
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
