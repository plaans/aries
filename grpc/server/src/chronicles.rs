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
use aries_planning::chronicles::printer::Printer;
use aries_planning::chronicles::VarType::StateVariableRead;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_utils::input::Sym;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
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

        // process types declared in the problem
        for tpe in &problem.types {
            let parent = if !tpe.parent_type.is_empty() {
                Some(Sym::from(&tpe.parent_type))
            } else {
                None
            };
            let type_name = Sym::from(&tpe.type_name);
            types.push((type_name, parent));
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
        if name == "up:bool" {
            Ok(Type::Bool)
        } else if name == "up:integer" {
            Ok(Type::Int)
        } else if name.starts_with("up:real") {
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

    // Initial chronicle construction
    let init_ch = Chronicle {
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

    let mut factory = ChronicleFactory {
        context: &mut context,
        chronicle: init_ch,
        container: Container::Base,
        parameters: Default::default(),
        variables: vec![],
    };

    // initial state is converted as a set of effects at the initial time
    for init_state in &problem.initial_state {
        let state_var = init_state
            .fluent
            .as_ref()
            .context("Initial state assignment has no valid fluent")?;
        let value = init_state
            .value
            .as_ref()
            .context("Initial state assignment has no valid value")?;
        let init_time = Span::instant(factory.chronicle.start);

        factory.add_effect(init_time, state_var, value, EffectKind::Assign)?;
    }

    // goals translate as condition at the global end time
    for goal in &problem.goals {
        let span = if let Some(itv) = &goal.timing {
            factory.read_time_interval(itv)?
        } else {
            Span::instant(factory.chronicle.end)
        };
        if let Some(goal) = &goal.goal {
            factory.enforce(goal, Some(span))?;
        }
    }

    let init_ch = factory.build_instance(ChronicleOrigin::Original)?;

    Printer::print_chronicle(&init_ch.chronicle, &context.model);

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

    println!("=== Instances ===");
    for ch in &problem.chronicles {
        Printer::print_chronicle(&ch.chronicle, &problem.context.model);
    }
    println!("=== Templates ===");
    for ch in &problem.templates {
        Printer::print_chronicle(&ch.chronicle, &problem.context.model);
    }

    Ok(problem)
}

fn str_to_symbol(name: &str, symbol_table: &SymbolTable) -> anyhow::Result<SAtom> {
    let sym = symbol_table
        .id(name)
        .with_context(|| format!("Unknown symbol / operator `{}`", name))?;
    let tpe = symbol_table.type_of(sym);
    Ok(SAtom::new_constant(sym, tpe))
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

#[derive(Copy, Clone, Debug)]
struct Span {
    start: Time,
    end: Time,
}
impl Span {
    pub fn interval(start: Time, end: Time) -> Span {
        Span { start, end }
    }
    pub fn instant(time: Time) -> Span {
        Span::interval(time, time)
    }
}

struct ChronicleFactory<'a> {
    context: &'a mut Ctx,
    chronicle: Chronicle,
    container: Container,
    parameters: HashMap<String, Variable>,
    variables: Vec<Variable>,
}

impl<'a> ChronicleFactory<'a> {
    pub fn build_template(self, label: String) -> Result<ChronicleTemplate, Error> {
        Ok(ChronicleTemplate {
            label: Some(label),
            parameters: self.variables,
            chronicle: self.chronicle,
        })
    }

    pub fn build_instance(self, origin: ChronicleOrigin) -> Result<ChronicleInstance, Error> {
        Ok(ChronicleInstance {
            parameters: self.variables.iter().map(|&v| v.into()).collect(),
            origin,
            chronicle: self.chronicle,
        })
    }

    fn parameter(&self, name: &str) -> Result<Atom, Error> {
        let var = *self
            .parameters
            .get(name)
            .with_context(|| format!("Unknown parameter: {name}"))?;
        Ok(var.into())
    }

    fn add_effect(
        &mut self,
        span: Span,
        state_var: &Expression,
        value: &Expression,
        kind: EffectKind,
    ) -> Result<(), Error> {
        // start of the effect, this is the one that is used to evaluate complex expression
        // (e.g. when a state variable is read inside the effect expression)
        let eff_start = Span::instant(span.start);

        let sv = self.read_state_variable(state_var, Some(eff_start))?;
        let value = self.reify(value, Some(eff_start))?;
        match kind {
            EffectKind::Assign => self.chronicle.effects.push(Effect {
                transition_start: span.start,
                persistence_start: span.end,
                state_var: sv,
                value,
            }),
            EffectKind::Increase | EffectKind::Decrease => bail!("Unsupported effect kind: {:?}", kind),
        }
        Ok(())
    }

    fn add_state_variable_read(
        &mut self,
        state_var: Sv,
        span: Span,
        expected_value: Option<Atom>,
    ) -> Result<Atom, Error> {
        let value = if let Some(value) = expected_value {
            value
        } else {
            // TODO: this would only support boolean state variables
            let value = self
                .context
                .model
                .new_optional_bvar(self.chronicle.presence, VarLabel(self.container, StateVariableRead));
            self.variables.push(value.into());
            value.true_lit().into()
        };

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
        self.bind_to(expr, Lit::TRUE.into(), span)
    }

    fn bind_to(&mut self, expr: &Expression, value: Atom, span: Option<Span>) -> Result<(), Error> {
        let expr_kind = ExpressionKind::from_i32(expr.kind).unwrap();
        match expr_kind {
            ExpressionKind::StateVariable => {
                let sv = self.read_state_variable(expr, span)?;
                ensure!(span.is_some(), "No temporal qualifier on state variable access.");
                self.add_state_variable_read(sv, span.unwrap(), Some(value))?;
            }
            ExpressionKind::FunctionApplication => {
                ensure!(
                    expr.atom.is_none(),
                    "Value Expression of type `FunctionApplication` should not have an atom"
                );

                // First element is going to be the function symbol, the rest are the parameters.
                let operator = as_function_symbol(&expr.list[0])?;
                let params = &expr.list[1..];

                match operator {
                    "up:eq" => {
                        let params: Vec<Atom> = params
                            .iter()
                            .map(|param| self.reify(param, span))
                            .collect::<Result<Vec<_>, _>>()?;
                        ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                        let value = Lit::try_from(value)?;
                        self.chronicle
                            .constraints
                            .push(Constraint::reified_eq(params[0], params[1], value));
                    }
                    "up:and" if value == Atom::TRUE => {
                        for p in params {
                            self.bind_to(p, value, span)?;
                        }
                    }
                    "up:not" => {
                        ensure!(params.len() == 1, "`not` operator should have exactly 1 argument");
                        let not_value = !Lit::try_from(value)?;
                        self.bind_to(&params[0], not_value.into(), span)?;
                    }
                    _ => bail!("Unsupported operator {operator}"),
                }
            }
            _ if value == Lit::TRUE.into() => {
                let reified = self.reify(expr, span)?;
                self.chronicle.constraints.push(Constraint::atom(reified));
            }
            _ => {
                let reified = self.reify(expr, span)?;
                self.chronicle.constraints.push(Constraint::eq(reified, value))
            }
        }

        Ok(())
    }

    fn reify(&mut self, expr: &aries_grpc_api::Expression, span: Option<Span>) -> Result<Atom, Error> {
        let expr_kind = ExpressionKind::from_i32(expr.kind).unwrap();
        use ExpressionKind::*;
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
                ensure!(span.is_some(), "No temporal qualifier on state variable access.");
                self.add_state_variable_read(sv, span.unwrap(), None)
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
                    "up:eq" => {
                        ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                        let reif = self.reify_equality(params[0], params[1]);
                        Ok(reif)
                    }
                    "up:not" => {
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
        Ok(Span::interval(start, end))
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
            .new_optional_sym_var(tpe, prez, container / VarType::Parameter(param.name.clone()));
        params.push(arg.into());
        name.push(arg.into());

        // Add parameters to the mapping
        parameter_mapping.insert(param.name.clone(), arg.into());
    }

    let ch = Chronicle {
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

    let mut factory = ChronicleFactory {
        context,
        chronicle: ch,
        container,
        parameters: parameter_mapping,
        variables: params,
    };

    // Process the effects of the action
    for eff in &action.effects {
        let timing = if let Some(occurrence) = &eff.occurrence_time {
            factory.read_timing(occurrence)?
        } else {
            factory.chronicle.end
        };
        let effect_span = Span::interval(timing, timing + FAtom::EPSILON);
        let eff = eff
            .effect
            .as_ref()
            .with_context(|| format!("Effect has no associated expression {eff:?}"))?;
        let sv = eff
            .fluent
            .as_ref()
            .with_context(|| format!("Effect expression has no fluent: {eff:?}"))?;
        let value = eff
            .value
            .as_ref()
            .with_context(|| format!("Effect has no value: {eff:?}"))?;

        let effect_kind =
            EffectKind::from_i32(eff.kind).with_context(|| format!("Unknown effect kind: {}", eff.kind))?;
        factory.add_effect(effect_span, sv, value, effect_kind)?;
    }

    for condition in &action.conditions {
        let span = if let Some(itv) = &condition.span {
            factory.read_time_interval(itv)?
        } else {
            Span::instant(factory.chronicle.start)
        };
        if let Some(cond) = &condition.cond {
            factory.enforce(cond, Some(span))?;
        }
    }

    factory.build_template(action.name.clone())
}
