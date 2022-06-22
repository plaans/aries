use anyhow::{anyhow, bail, ensure, Context, Error, Ok};
use aries_core::{IntCst, Lit, INT_CST_MAX};
use aries_grpc_api::atom::Content;
use aries_grpc_api::effect_expression::EffectKind;
use aries_grpc_api::timepoint::TimepointKind;
use aries_grpc_api::{Expression, ExpressionKind, Problem};
use aries_model::extensions::Shaped;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_planning::chronicles::constraints::Constraint;
use aries_planning::chronicles::VarType::StateVariableRead;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_utils::input::Sym;
use std::collections::HashMap;
use std::convert::TryInto;
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
                    format!(
                        "Invalid parameter type `{}` for fluent parameter `{}`",
                        arg.r#type, arg.name
                    )
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
    let scope = Scope {
        variables: HashMap::new(),
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

        let (state_var, value) = read_initial_assignment(expr, value, &scope, &context)?;

        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var,
            value,
        })
    }

    let mut env = ChronicleFactory {
        context: &mut context,
        chronicle: &mut init_ch,
        container: Container::Base,
        parameters: Default::default(),
        variables: vec![],
    };

    // goals translate as condition at the global end time
    println!("===== Goals =====");
    for goal in &problem.goals {
        let span = if let Some(itv) = &goal.timing {
            env.read_time_interval(itv)?
        } else {
            Span {
                start: env.chronicle.end,
                end: env.chronicle.end,
            }
        };
        if let Some(goal) = &goal.goal {
            env.enforce(goal, Some(span))?;
        }
    }
    println!("===== Initial Chronicle =====");
    dbg!(&init_ch);

    // TODO: Task networks?
    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    dbg!(&init_ch.chronicle);

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
fn read_initial_assignment(
    expr: &Expression,
    value: &Expression,
    scope: &Scope,
    context: &Ctx,
) -> Result<(Sv, Atom), Error> {
    let sv = read_state_variable(expr, scope, context)?;
    let value = read_value(value, scope, context)?;
    Ok((sv, value))
}

fn read_atom(atom: &aries_grpc_api::Atom, symbol_table: &SymbolTable) -> Result<aries_model::lang::Atom, Error> {
    if let Some(atom_content) = atom.content.clone() {
        match atom_content {
            aries_grpc_api::atom::Content::Symbol(s) => {
                let atom = str_to_symbol(s.as_str(), symbol_table)?;
                Ok(atom.into())
            }
            aries_grpc_api::atom::Content::Int(i) => Ok(Atom::from(i)),
            aries_grpc_api::atom::Content::Real(_f) => {
                bail!("`Real` type not supported yet")
            }
            aries_grpc_api::atom::Content::Boolean(b) => Ok(Atom::Bool(b.into())),
        }
    } else {
        bail!("Unsupported atom")
    }
}

/// Read the expression and return the state variables for the expressions
/// The expression parameters can be of type `FluentSymbol` or `StateVariable` or `Parameter`
/// The expression type `FunctionApplication` should hold the following in order:
/// - FunctionSymbol
/// - List of parameters (FluentSymbol or StateVariable or Parameter)
fn read_state_variable(expr: &Expression, scope: &Scope, context: &Ctx) -> Result<Sv, Error> {
    let mut sv = Vec::new();
    let expr_kind = ExpressionKind::from_i32(expr.kind).unwrap();

    if expr_kind == ExpressionKind::Constant {
        Ok(vec![read_atom(
            expr.atom.as_ref().unwrap(),
            context.model.get_symbol_table(),
        )?
        .try_into()
        .unwrap()])
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
                                let state_var = read_state_variable(sub_expr, scope, context)?;
                                sv.extend(state_var);
                            }
                        }
                        "and" => {
                            todo!("`and` operator not supported yet")
                        }
                        "not" => { // TODO: If `not` state variables are empty while the values are altered
                        }
                        _ => {
                            bail!("Unsupported operator `{}`", operator)
                        }
                    }
                } else {
                    bail!("Operator {:?} should be a symbol", operator);
                }
            } else {
                let state_var = read_state_variable(&sub_expr, scope, context)?;
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
                    Atom::Sym(fluent) => sv.push(fluent),
                    _ => bail!("Expected a valid fluent symbol as atom in expression"),
                }
            } else {
                let state_var = read_state_variable(&sub_expr, scope, context)?;
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

#[derive(Copy, Clone, Debug)]
struct Span {
    start: Time,
    end: Time,
}
impl Span {
    pub fn new(start: Time, end: Time) -> Span {
        Span { start, end }
    }
}

struct ChronicleFactory<'a> {
    context: &'a mut Ctx,
    chronicle: &'a mut Chronicle,
    container: Container,
    parameters: HashMap<String, Variable>,
    variables: Vec<Variable>,
}

impl<'a> ChronicleFactory<'a> {
    fn parameter(&self, name: &str) -> Result<Atom, Error> {
        let var = *self
            .parameters
            .get(name)
            .with_context(|| format!("Unknown parameter: {name}"))?;
        Ok(var.into())
    }

    fn add_state_variable_read(&mut self, state_var: Sv, span: Span) -> Result<Atom, Error> {
        // TODO: this would only support boolean state variables
        let value = self
            .context
            .model
            .new_optional_bvar(self.chronicle.presence, VarLabel(self.container, StateVariableRead));
        self.variables.push(value.into());
        let value = value.true_lit().into();
        let condition = Condition {
            start: span.start,
            end: span.end,
            state_var,
            value,
        };
        self.chronicle.conditions.push(condition);
        Ok(value)
    }

    fn reify_equality(&mut self, a: Atom, b: Atom) -> Atom {
        let value = self
            .context
            .model
            .new_optional_bvar(self.chronicle.presence, VarLabel(self.container, StateVariableRead));
        self.variables.push(value.into());
        let value = value.true_lit();
        self.chronicle.constraints.push(Constraint::reified_eq(a, b, value));
        value.into()
    }

    fn enforce(&mut self, expr: &aries_grpc_api::Expression, span: Option<Span>) -> Result<(), Error> {
        let reified = self.reify(expr, span)?;
        self.chronicle.constraints.push(Constraint::atom(reified));
        Ok(())
    }

    fn reify(&mut self, expr: &aries_grpc_api::Expression, span: Option<Span>) -> Result<Atom, Error> {
        let expr_kind = ExpressionKind::from_i32(expr.kind).unwrap();
        use ExpressionKind::*;
        // dbg!(expr);
        match expr_kind {
            Constant => {
                let atom = expr.atom.as_ref().context("Malformed protobuf: expected an atom")?;
                read_atom(atom, self.context.model.get_symbol_table()).with_context(|| format!("Unknown atom {atom:?}"))
            }
            Parameter => {
                ensure!(expr.atom.is_some(), "Parameter should have an atom");
                let parameter_name = expr.atom.as_ref().unwrap().content.as_ref().unwrap();
                match parameter_name {
                    aries_grpc_api::atom::Content::Symbol(s) => self.parameter(s.as_str()),
                    _ => bail!("Parameter should be a symbol: {expr:?}"),
                }
            }
            ExpressionKind::StateVariable => {
                let sv = self.read_state_variable(expr, span)?;
                ensure!(span.is_some(), "Not temporal qualifier on state variable access.");
                self.add_state_variable_read(sv, span.unwrap())
            }
            FunctionApplication => {
                ensure!(
                    expr.atom.is_none(),
                    "Value Expression of type `FunctionApplication` should not have an atom"
                );

                // First element is going to be function symbol, the rest are the parameters.
                let operator = as_function_symbol(&expr.list[0])?;
                let params = &expr.list[1..];
                let params: Vec<Atom> = params
                    .iter()
                    .map(|param| self.reify(param, span))
                    .collect::<Result<_, _>>()?;

                match operator {
                    "=" => {
                        ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                        let reif = self.reify_equality(params[0], params[1]);
                        Ok(reif)
                    }
                    "not" => {
                        ensure!(params.len() == 1, "`not` operator should have exactly 1 argument");
                        let param: Lit = params[0].try_into()?;
                        Ok(Atom::Bool(!param))
                    }
                    _ => bail!("Unsupported operator {operator}"),
                }
            }
            _ => unimplemented!("expression kind: {expr_kind:?}"),
        }
    }

    fn read_state_variable(&mut self, expr: &Expression, span: Option<Span>) -> Result<Sv, Error> {
        ensure!(
            expr.atom.is_none(),
            "Value Expression of type `StateVariable` should not have an atom"
        );
        ensure!(!expr.list.is_empty(), "Empty state variable expression");
        let mut sv = Vec::with_capacity(expr.list.len());
        let fluent = self.read_fluent_symbol(&expr.list[0])?;
        sv.push(fluent);
        for arg in &expr.list[1..] {
            let arg = self.reify(arg, span)?;
            let arg: SAtom = arg
                .try_into()
                .with_context(|| format!("Non-symbolic atom in state variable {arg:?}."))?;
            sv.push(arg);
        }
        Ok(sv)
    }

    fn read_timing(&self, timing: &aries_grpc_api::Timing) -> Result<FAtom, Error> {
        let (delay_num, delay_denom) = {
            let (num, denom) = if let Some(delay) = timing.delay.as_ref() {
                (delay.numerator, delay.denominator)
            } else {
                (0, 1)
            };
            let num: IntCst = num
                .try_into()
                .context("Only 32 bits integers supported in Rational numbers")?;
            let denom: IntCst = denom
                .try_into()
                .context("Only 32 bits integers supported in Rational numbers")?;
            ensure!(TIME_SCALE % denom == 0, "Time scale beyond what is supported.");
            let scale = TIME_SCALE / denom;
            (num * scale, denom * scale)
        };
        let kind = if let Some(timepoint) = timing.timepoint.as_ref() {
            TimepointKind::from_i32(timepoint.kind).context("Unsupported timepoint kind")?
        } else {
            // not time point specified, interpret as 0.
            TimepointKind::GlobalStart
        };
        let tp = match kind {
            TimepointKind::GlobalStart => self.context.origin(),
            TimepointKind::GlobalEnd => self.context.horizon(),
            TimepointKind::Start => self.chronicle.start,
            TimepointKind::End => self.chronicle.end,
        };
        assert_eq!(tp.denom, delay_denom);
        Ok(FAtom::new(tp.num + delay_num, tp.denom))
    }

    fn read_time_interval(&self, interval: &aries_grpc_api::TimeInterval) -> Result<Span, Error> {
        let interval = interval.clone();
        let start = self.read_timing(&interval.lower.unwrap())?;
        let end = self.read_timing(&interval.upper.unwrap())?;
        Ok(Span { start, end })
    }

    fn read_fluent_symbol(&self, expr: &Expression) -> Result<SAtom, Error> {
        ensure!(expr.kind == ExpressionKind::FluentSymbol as i32);

        match read_atom(expr.atom.as_ref().unwrap(), self.context.model.get_symbol_table())? {
            Atom::Sym(fluent) => Ok(fluent),
            x => bail!("Not a symbol {x:?}"),
        }
    }
}

fn as_function_symbol(expr: &Expression) -> Result<&str, Error> {
    ensure!(
        expr.kind == ExpressionKind::FunctionSymbol as i32,
        "Expected function symbol: {expr:?}"
    );
    as_symbol(expr)
}

fn as_symbol(expr: &Expression) -> Result<&str, Error> {
    let atom = expr
        .atom
        .as_ref()
        .with_context(|| format!("Expected a symbol: {expr:?}"))?
        .content
        .as_ref()
        .with_context(|| "Missing content")?;
    match atom {
        Content::Symbol(sym) => Ok(sym.as_str()),
        _ => bail!("Expected symbol but got: {atom:?}"),
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
/// BUG: in reading values
fn read_value(expr: &aries_grpc_api::Expression, scope: &Scope, context: &Ctx) -> Result<Atom, Error> {
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
                let var = scope
                    .variables
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
        Ok(atom)
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
                        Ok(value)
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
        } else {
            read_value(&expr_head, scope, context)
        }
    } else {
        bail!("Unsupported expression kind: {:?}", expr_kind)
    }
}

struct Scope {
    variables: HashMap<String, Variable>,
}

fn read_chronicle_template(
    container: Container,
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
    let prez_var = context.model.new_bvar(container / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();
    let mut parameter_mapping: HashMap<String, Variable> = HashMap::new();

    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, container / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = match action_kind {
        ChronicleKind::Problem => bail!("Problem type not supported"),
        ChronicleKind::Method | ChronicleKind::DurativeAction => {
            let end =
                context
                    .model
                    .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, container / VarType::ChronicleEnd);
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
        let arg = context
            .model
            .new_optional_sym_var(tpe, prez, container / VarType::Parameter); // arg.symbol
        params.push(arg.into());
        name.push(arg.into());

        // Add parameters to the mapping
        parameter_mapping.insert(param.name.clone(), arg.into());
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

    let mut env = ChronicleFactory {
        context,
        chronicle: &mut ch,
        container,
        parameters: parameter_mapping,
        variables: params,
    };

    // Process the effects of the action
    for eff in &action.effects {
        let timing = if let Some(occurrence) = &eff.occurrence_time {
            env.read_timing(occurrence)?
        } else {
            env.chronicle.end
        };
        let read_span = Span::new(timing, timing);
        let eff = eff
            .effect
            .as_ref()
            .with_context(|| format!("Effect has no associated expression {eff:?}"))?;
        let sv = eff
            .fluent
            .as_ref()
            .with_context(|| format!("Effect expression has no fluent: {eff:?}"))?;
        let sv = env.read_state_variable(sv, Some(read_span))?;
        let value = eff
            .value
            .as_ref()
            .with_context(|| format!("Effect has no value: {eff:?}"))?;
        let value = env.reify(value, Some(read_span))?;
        // ensure!(eff.condition.is_none(), "Unsupported conditional effect: {eff:?}");
        match EffectKind::from_i32(eff.kind) {
            Some(EffectKind::Assign) => env.chronicle.effects.push(Effect {
                transition_start: timing,
                persistence_start: timing + FAtom::EPSILON,
                state_var: sv,
                value,
            }),
            Some(x) => bail!("Unsupported effect kind: {x:?}"),
            None => bail!("Unknown effect kind"),
        }
    }

    for condition in &action.conditions {
        let span = if let Some(itv) = &condition.span {
            env.read_time_interval(itv)?
        } else {
            Span {
                start: env.chronicle.start,
                end: env.chronicle.start,
            }
        };
        if let Some(cond) = &condition.cond {
            env.enforce(cond, Some(span))?;
        }
    }

    println!("===");
    dbg!(&env.chronicle);
    println!("===");

    Ok(ChronicleTemplate {
        label: Some(action.name.clone()),
        parameters: env.variables,
        chronicle: ch,
    })
}
