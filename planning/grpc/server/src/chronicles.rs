use anyhow::{anyhow, bail, ensure, Context, Error, Ok};
use aries::core::{IntCst, Lit, INT_CST_MAX, INT_CST_MIN};
use aries::model::extensions::Shaped;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::*;
use aries::model::symbols::SymbolTable;
use aries::model::types::TypeHierarchy;
use aries::utils::input::Sym;
use aries_planning::chronicles::constraints::{Constraint, ConstraintType, Duration};
use aries_planning::chronicles::VarType::Reification;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use regex::Regex;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use unified_planning as up;
use up::atom::Content;
use up::effect_expression::EffectKind;
use up::metric::MetricKind;
use up::timepoint::TimepointKind;
use up::{Action, Expression, ExpressionKind, Problem};

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
            let object_symbol = Sym::from(&obj.name);
            let object_type = Sym::from(&obj.r#type);

            // declare the object as a new symbol with the given type
            symbols.push(TypedSymbol {
                symbol: object_symbol,
                tpe: Some(object_type),
            });
        }

        // record all symbols representing fluents
        for fluent in &problem.fluents {
            symbols.push(TypedSymbol {
                symbol: Sym::from(&fluent.name),
                tpe: Some(FLUENT_TYPE.into()),
            });
        }

        // actions are symbols as well, add them to the table
        for action in &problem.actions {
            symbols.push(TypedSymbol {
                symbol: Sym::from(&action.name),
                tpe: Some(ACTION_TYPE.into()),
            });
        }

        if let Some(hierarchy) = &problem.hierarchy {
            for task in &hierarchy.abstract_tasks {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(&task.name),
                    tpe: Some(ABSTRACT_TASK_TYPE.into()),
                })
            }

            for method in &hierarchy.methods {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(&method.name),
                    tpe: Some(METHOD_TYPE.into()),
                })
            }
        }
    }

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(types.clone(), symbols)?;

    let int_type_regex = Regex::new(r"^up:integer\[(-?\d+)\s*,\s*(-?\d+)\]$").unwrap();

    let from_upf_type = |name: &str| {
        if name == "up:bool" {
            Ok(Type::Bool)
        } else if name == "up:integer" {
            // integer type with no bounds
            Ok(Type::UNBOUNDED_INT)
        } else if let Some(x) = int_type_regex.captures_iter(name).next() {
            // integer type with bounds
            let lb: IntCst = x[1].parse().unwrap();
            let ub: IntCst = x[2].parse().unwrap();
            ensure!(lb <= ub, "Invalid bounds [{lb}, {ub}]");
            ensure!(lb >= INT_CST_MIN, "Int lower bound is too small: {lb}");
            ensure!(ub <= INT_CST_MAX, "Int upper bound is too big: {ub}");
            Ok(Type::Int { lb, ub })
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
            let mut signature = Vec::with_capacity(1 + fluent.parameters.len());

            for arg in &fluent.parameters {
                signature.push(from_upf_type(arg.r#type.as_str()).with_context(|| {
                    format!(
                        "Invalid parameter type `{}` for fluent parameter `{}`",
                        arg.r#type, arg.name
                    )
                })?);
            }

            signature.push(from_upf_type(&fluent.value_type)?);

            state_variables.push(Fluent { sym, signature });
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
        cost: None,
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
            factory
                .read_time_interval(itv)
                .with_context(|| format!("In time interval of goal: {goal:?}"))?
        } else {
            Span::instant(factory.chronicle.end)
        };
        if let Some(goal) = &goal.goal {
            factory
                .enforce(goal, Some(span))
                .with_context(|| format!("In goal expression {goal}",))?;
        }
    }

    if let Some(hierarchy) = &problem.hierarchy {
        let tn = hierarchy
            .initial_task_network
            .as_ref()
            .context("Missing initial task network in hierarchical problem")?;
        for var in &tn.variables {
            factory.add_parameter(&var.name, &var.r#type)?;
        }

        for subtask in &tn.subtasks {
            factory.add_subtask(subtask)?;
        }

        for constraint in &tn.constraints {
            factory
                .enforce(constraint, None)
                .with_context(|| format!("In initial task network constraint: {constraint}"))?;
        }
    }

    let init_ch = factory.build_instance(ChronicleOrigin::Original)?;

    ensure!(problem.metrics.len() <= 1, "No support for multiple metrics.");
    let action_costs = problem
        .metrics
        .iter()
        .find(|metric| MetricKind::from_i32(metric.kind) == Some(MetricKind::MinimizeActionCosts));
    let action_costs = if let Some(metric) = action_costs {
        ActionCosts {
            costs: metric.action_costs.clone(),
            default: metric.default_action_cost.clone(),
        }
    } else {
        ActionCosts {
            costs: HashMap::new(),
            default: None,
        }
    };

    let mut templates = Vec::new();
    for a in &problem.actions {
        let cont = Container::Template(templates.len());
        let template = read_action(cont, a, &action_costs, &mut context)?;
        templates.push(template);
    }

    if let Some(hierarchy) = &problem.hierarchy {
        for method in &hierarchy.methods {
            let cont = Container::Template(templates.len());
            let template = read_method(cont, method, &mut context)?;
            templates.push(template);
        }
    }

    let problem = aries_planning::chronicles::Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    // println!("=== Instances ===");
    // for ch in &problem.chronicles {
    //     Printer::print_chronicle(&ch.chronicle, &problem.context.model);
    // }
    // println!("=== Templates ===");
    // for ch in &problem.templates {
    //     Printer::print_chronicle(&ch.chronicle, &problem.context.model);
    // }

    Ok(problem)
}

struct ActionCosts {
    costs: HashMap<String, Expression>,
    default: Option<Expression>,
}

fn str_to_symbol(name: &str, symbol_table: &SymbolTable) -> anyhow::Result<SAtom> {
    let sym = symbol_table
        .id(name)
        .with_context(|| format!("Unknown symbol / operator `{name}`"))?;
    let tpe = symbol_table.type_of(sym);
    Ok(SAtom::new_constant(sym, tpe))
}

fn read_atom(atom: &up::Atom, symbol_table: &SymbolTable) -> Result<aries::model::lang::Atom, Error> {
    if let Some(atom_content) = atom.content.clone() {
        match atom_content {
            up::atom::Content::Symbol(s) => {
                let atom = str_to_symbol(s.as_str(), symbol_table)?;
                Ok(atom.into())
            }
            up::atom::Content::Int(i) => Ok(Atom::from(i)),
            up::atom::Content::Real(_f) => {
                bail!("`Real` type not supported yet")
            }
            up::atom::Content::Boolean(b) => Ok(Atom::Bool(b.into())),
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

    fn add_parameter(&mut self, name: impl Into<Sym>, tpe: impl Into<Sym>) -> Result<SVar, Error> {
        let name = name.into();
        let tpe = tpe.into();
        let tpe = self
            .context
            .model
            .get_symbol_table()
            .types
            .id_of(&tpe)
            .ok_or_else(|| name.invalid("Unknown argument"))?;
        let arg = self.context.model.new_optional_sym_var(
            tpe,
            self.chronicle.presence,
            self.container / VarType::Parameter(name.to_string()),
        );

        // append parameters to the name of the chronicle
        self.chronicle.name.push(arg.into());

        self.variables.push(arg.into());

        // add parameters to the mapping
        let name_string = name.to_string();
        assert!(!self.parameters.contains_key(&name_string));
        self.parameters.insert(name_string, arg.into());

        Ok(arg)
    }

    fn create_timepoint(&mut self, vartype: VarType) -> FAtom {
        let tp = self.context.model.new_optional_fvar(
            0,
            INT_CST_MAX,
            TIME_SCALE,
            self.chronicle.presence,
            self.container / vartype,
        );
        self.variables.push(tp.into());
        FAtom::from(tp)
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
                min_persistence_end: Vec::new(),
                state_var: sv,
                value,
            }),
            EffectKind::Increase | EffectKind::Decrease => bail!("Unsupported effect kind: {:?}", kind),
        }
        Ok(())
    }

    fn create_variable(&mut self, tpe: Type, var_type: VarType) -> Variable {
        let var: Variable = match tpe {
            Type::Sym(tpe) => self
                .context
                .model
                .new_optional_sym_var(tpe, self.chronicle.presence, self.container / var_type)
                .into(),
            Type::Int { lb, ub } => self
                .context
                .model
                .new_optional_ivar(lb, ub, self.chronicle.presence, self.container / var_type)
                .into(),
            Type::Fixed(denom) => self
                .context
                .model
                .new_optional_fvar(
                    INT_CST_MIN,
                    INT_CST_MAX,
                    denom,
                    self.chronicle.presence,
                    self.container / var_type,
                )
                .into(),

            Type::Bool => self
                .context
                .model
                .new_optional_bvar(self.chronicle.presence, self.container / var_type)
                .into(),
        };
        self.variables.push(var);
        var
    }

    fn create_bool_variable(&mut self, label: VarType) -> Lit {
        let var = self
            .context
            .model
            .new_optional_bvar(self.chronicle.presence, self.container / label);
        self.variables.push(var.into());
        var.true_lit()
    }

    fn add_state_variable_read(
        &mut self,
        state_var: StateVar,
        span: Span,
        expected_value: Option<Atom>,
    ) -> Result<Atom, Error> {
        let value = if let Some(value) = expected_value {
            value
        } else {
            let value_type = state_var.fluent.return_type();
            let value = self.create_variable(value_type, Reification);
            value.into()
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

    fn add_condition(&mut self, condition: &up::Condition) -> Result<(), Error> {
        let span = if let Some(itv) = &condition.span {
            self.read_time_interval(itv)?
        } else {
            Span::instant(self.chronicle.start)
        };
        if let Some(cond) = &condition.cond {
            self.enforce(cond, Some(span))?;
        }
        Ok(())
    }

    fn set_cost(&mut self, cost: &Expression) -> Result<(), Error> {
        ensure!(kind(cost)? == ExpressionKind::Constant);
        ensure!(cost.r#type == "up:integer");
        let cost = match cost.atom.as_ref().unwrap().content.as_ref().unwrap() {
            Content::Int(i) => *i as IntCst,
            _ => bail!("Unexpected cost type."),
        };
        self.chronicle.cost = Some(cost);
        Ok(())
    }

    fn add_subtask(&mut self, subtask: &up::Task) -> Result<(), Error> {
        let task_index = self.chronicle.subtasks.len() as u32;
        let start = self.create_timepoint(VarType::TaskStart(task_index));
        let end = self.create_timepoint(VarType::TaskEnd(task_index));
        let mut task_name = Vec::with_capacity(subtask.parameters.len() + 1);
        task_name.push(str_to_symbol(&subtask.task_name, &self.context.model.shape.symbols)?);
        for param in &subtask.parameters {
            let param = self.reify(param, None)?;
            let param: SAtom = param.try_into()?;
            task_name.push(param);
        }
        self.chronicle.subtasks.push(SubTask {
            id: Some(subtask.id.clone()),
            start,
            end,
            task_name,
        });
        Ok(())
    }

    fn reify_equality(&mut self, a: Atom, b: Atom) -> Atom {
        let value = self
            .context
            .model
            .new_optional_bvar(self.chronicle.presence, self.container / Reification);
        self.variables.push(value.into());
        let value = value.true_lit();
        self.chronicle.constraints.push(Constraint::reified_eq(a, b, value));
        value.into()
    }

    fn enforce(&mut self, expr: &up::Expression, span: Option<Span>) -> Result<(), Error> {
        self.bind_to(expr, Lit::TRUE.into(), span) // TODO: use scope's tautology
    }

    fn bind_to(&mut self, expr: &Expression, value: Atom, span: Option<Span>) -> Result<(), Error> {
        match kind(expr)? {
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
                    "up:equals" => {
                        ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                        let params: Vec<Atom> = params
                            .iter()
                            .map(|param| self.reify(param, span))
                            .collect::<Result<Vec<_>, _>>()?;
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
                    "up:or" => {
                        let params: Vec<Atom> = params
                            .iter()
                            .map(|param| self.reify(param, span))
                            .collect::<Result<Vec<_>, _>>()?;
                        let value = Lit::try_from(value)?;
                        self.chronicle.constraints.push(Constraint {
                            variables: params,
                            tpe: ConstraintType::Or,
                            value: Some(value),
                        })
                    }
                    "up:not" => {
                        ensure!(params.len() == 1, "`not` operator should have exactly 1 argument");
                        let not_value = !Lit::try_from(value)?;
                        self.bind_to(&params[0], not_value.into(), span)?;
                    }
                    "up:lt" => {
                        ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                        let params: Vec<Atom> = params
                            .iter()
                            .map(|param| self.reify(param, span))
                            .collect::<Result<Vec<_>, _>>()?;

                        let value = Lit::try_from(value)?;
                        self.chronicle
                            .constraints
                            .push(Constraint::reified_lt(params[0], params[1], value));
                    }
                    _ => bail!("Unsupported operator binding: {operator}"),
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

    fn reify(&mut self, expr: &up::Expression, span: Option<Span>) -> Result<Atom, Error> {
        use ExpressionKind::*;
        match kind(expr)? {
            Constant => {
                let atom = expr.atom.as_ref().context("Malformed protobuf: expected an atom")?;
                read_atom(atom, self.context.model.get_symbol_table()).with_context(|| format!("Unknown atom {atom:?}"))
            }
            Parameter => {
                ensure!(expr.atom.is_some(), "Parameter should have an atom");
                let parameter_name = expr.atom.as_ref().unwrap().content.as_ref().unwrap();
                match parameter_name {
                    up::atom::Content::Symbol(s) => self.parameter(s.as_str()),
                    _ => bail!("Parameter should be a symbol: {expr:?}"),
                }
            }
            StateVariable => {
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

                if operator == "up:start"
                    || operator == "up:end"
                    || operator == "up:global_start"
                    || operator == "up:global_end"
                {
                    // extract the timepoint
                    ensure!(
                        params.len() <= 1,
                        "Too many parameters for temporal qualifier: {operator}"
                    );
                    let timepoint = if let Some(param) = params.get(0) {
                        // we must have something of the form `up:start(task_id)` or `up:end(task_id)`
                        ensure!(kind(param)? == ExpressionKind::ContainerId);
                        let container = match param.atom.as_ref().unwrap().content.as_ref().unwrap() {
                            Content::Symbol(name) => name,
                            _ => bail!("Malformed protobuf"),
                        };
                        let subtask = self
                            .chronicle
                            .subtasks
                            .iter()
                            .find(|subtask| subtask.id.as_ref() == Some(container));
                        let subtask = subtask.with_context(|| format!("Unknown task id: {container}"))?;
                        match operator {
                            "up:start" => subtask.start,
                            "up:end" => subtask.end,
                            x => bail!("Time extractor {x} has an unexpected parameter. "),
                        }
                    } else {
                        match operator {
                            "up:start" => self.chronicle.start,
                            "up:end" => self.chronicle.end,
                            "up:global_start" => self.context.origin(),
                            "up:global_end" => self.context.horizon(),
                            _ => unreachable!(),
                        }
                    };
                    Ok(timepoint.into())
                } else {
                    let params: Vec<Atom> = params
                        .iter()
                        .map(|param| self.reify(param, span))
                        .collect::<Result<_, _>>()?;

                    match operator {
                        "up:or" => {
                            let value = self.create_bool_variable(VarType::Reification);
                            let constraint = Constraint {
                                variables: params,
                                tpe: ConstraintType::Or,
                                value: Some(value),
                            };
                            self.chronicle.constraints.push(constraint);
                            Ok(value.into())
                        }
                        "up:and" => {
                            // convert (and a b c) into  !(or !a !b !c)
                            let mut disjuncts = Vec::with_capacity(params.len());
                            for param in params {
                                let param =
                                    Lit::try_from(param).context("`up:and` expression has a non boolean parameter")?;
                                let disjunct = !param;
                                disjuncts.push(disjunct.into());
                            }
                            let value = self.create_bool_variable(VarType::Reification);
                            let constraint = Constraint {
                                variables: disjuncts,
                                tpe: ConstraintType::Or,
                                value: Some(value),
                            };
                            self.chronicle.constraints.push(constraint);
                            Ok((!value).into())
                        }
                        "up:equals" => {
                            ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                            let reif = self.reify_equality(params[0], params[1]);
                            Ok(reif)
                        }
                        "up:not" => {
                            ensure!(params.len() == 1, "`not` operator should have exactly 1 argument");
                            let param: Lit = params[0].try_into()?;
                            Ok(Atom::Bool(!param))
                        }
                        "up:lt" => {
                            ensure!(params.len() == 2, "`<` operator should have exactly 2 arguments");
                            let value = self.create_bool_variable(VarType::Reification);
                            let constraint = Constraint {
                                variables: params,
                                tpe: ConstraintType::Lt,
                                value: Some(value),
                            };
                            self.chronicle.constraints.push(constraint);
                            Ok(value.into())
                        }
                        _ => bail!("Unsupported operator {operator}"),
                    }
                }
            }
            kind => unimplemented!("expression kind: {kind:?}"),
        }
    }

    fn read_state_variable(&mut self, expr: &Expression, span: Option<Span>) -> Result<StateVar, Error> {
        ensure!(
            expr.atom.is_none(),
            "Value Expression of type `StateVariable` should not have an atom"
        );
        ensure!(!expr.list.is_empty(), "Empty state variable expression");

        let fluent = self.read_fluent_symbol(&expr.list[0])?;
        let mut args = Vec::with_capacity(expr.list.len());
        for arg in &expr.list[1..] {
            let arg = self.reify(arg, span)?;
            let arg: SAtom = arg
                .try_into()
                .with_context(|| format!("Non-symbolic atom in state variable {arg:?}."))?;
            args.push(arg);
        }
        Ok(StateVar::new(fluent, args))
    }

    fn read_timing(&self, timing: &up::Timing) -> Result<FAtom, Error> {
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

    /// Returns the corresponding start and end timepoints representing the interval.
    /// Note that if the interval left/right opened, the corresponding timepoint is shifted by the smallest representable value.
    fn read_time_interval(&self, interval: &up::TimeInterval) -> Result<Span, Error> {
        let start = self.read_timing(interval.lower.as_ref().unwrap())?;
        let start = if interval.is_left_open {
            start + FAtom::EPSILON
        } else {
            start
        };
        let end = self.read_timing(interval.upper.as_ref().unwrap())?;
        let end = if interval.is_right_open {
            end - FAtom::EPSILON
        } else {
            end
        };
        Ok(Span::interval(start, end))
    }

    fn read_fluent_symbol(&self, expr: &Expression) -> Result<Arc<Fluent>, Error> {
        ensure!(kind(expr)? == ExpressionKind::FluentSymbol);

        match read_atom(expr.atom.as_ref().unwrap(), self.context.model.get_symbol_table())? {
            Atom::Sym(SAtom::Cst(sym)) => self.context.get_fluent(sym.sym).cloned().context("Unknown fluent"),
            x => bail!("Not a symbol {x:?}"),
        }
    }
}

fn as_function_symbol(expr: &Expression) -> Result<&str, Error> {
    ensure!(
        kind(expr)? == ExpressionKind::FunctionSymbol,
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

/// If the action has a fixed duration, returns it otherwise returns None
fn get_fixed_duration(action: &Action) -> Option<IntCst> {
    let duration = action.duration.as_ref()?;
    let ctl = duration.controllable_in_bounds.as_ref()?;
    let min = ctl.lower.as_ref()?;
    let max = ctl.upper.as_ref()?;
    if min == max && !ctl.is_left_open && !ctl.is_right_open {
        as_int(min).ok()
    } else {
        None
    }
}

fn read_action(
    container: Container,
    action: &up::Action,
    costs: &ActionCosts,
    context: &mut Ctx,
) -> Result<ChronicleTemplate, Error> {
    let action_kind = {
        if action.duration.is_some() {
            ChronicleKind::DurativeAction
        } else {
            ChronicleKind::Action
        }
    };
    let mut variables: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(container / VarType::Presence);
    variables.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, container / VarType::ChronicleStart);
    variables.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = match action_kind {
        ChronicleKind::Problem | ChronicleKind::Method => unreachable!(),
        ChronicleKind::DurativeAction => {
            if let Some(dur) = get_fixed_duration(action) {
                // a duration constraint is added later in the function for more complex durations
                start + dur
            } else {
                let end = context.model.new_optional_fvar(
                    0,
                    INT_CST_MAX,
                    TIME_SCALE,
                    prez,
                    container / VarType::ChronicleEnd,
                );
                variables.push(end.into());
                end.into()
            }
        }
        ChronicleKind::Action => start, // non-temporal actions are instantaneous
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
        cost: None,
    };

    let mut factory = ChronicleFactory {
        context,
        chronicle: ch,
        container,
        parameters: Default::default(),
        variables,
    };

    // process the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for param in &action.parameters {
        factory.add_parameter(&param.name, &param.r#type)?;
    }

    // set the action's achieved task to be the same as the its name
    // note that we wait until all parameters have been add to the name before doing this
    factory.chronicle.task = Some(factory.chronicle.name.clone());

    let mut affected_state_variables = vec![];
    // Process the effects of the action
    for eff in &action.effects {
        let effect_span = if let Some(occurrence) = &eff.occurrence_time {
            let start = factory.read_timing(occurrence)?;
            Span::interval(start, start + FAtom::EPSILON)
        } else {
            ensure!(
                action_kind == ChronicleKind::Action,
                "Durative action with untimed effect."
            );
            Span::interval(factory.chronicle.end, factory.chronicle.end + Time::EPSILON)
        };
        let eff = eff
            .effect
            .as_ref()
            .with_context(|| format!("Effect has no associated expression {eff:?}"))?;
        let sv = eff
            .fluent
            .as_ref()
            .with_context(|| format!("Effect expression has no fluent: {eff:?}"))?;
        affected_state_variables.push(sv);
        let value = eff
            .value
            .as_ref()
            .with_context(|| format!("Effect has no value: {eff:?}"))?;

        let effect_kind =
            EffectKind::from_i32(eff.kind).with_context(|| format!("Unknown effect kind: {}", eff.kind))?;
        factory.add_effect(effect_span, sv, value, effect_kind)?;
    }

    for condition in &action.conditions {
        // note: this is effectively `factory.add_condition(condition)` with a work around for mutex conditions in instantaneous actions
        if let Some(cond) = &condition.cond {
            let span = if let Some(itv) = &condition.span {
                factory.read_time_interval(itv)?
            } else {
                ensure!(
                    action_kind == ChronicleKind::Action,
                    "Durative action with untimed condition."
                );
                // We have no time span associated to this condition, which can only happen for a PDDL "instantaneous" actions.
                Span::interval(factory.chronicle.start, factory.chronicle.end)
            };
            factory.enforce(cond, Some(span))?;
        }
    }

    if let Some(duration) = action.duration.as_ref() {
        let start = factory.chronicle.start;
        if let Some(interval) = duration.controllable_in_bounds.as_ref() {
            let min = interval
                .lower
                .as_ref()
                .with_context(|| "Duration without a lower bound")?;
            let max = interval
                .upper
                .as_ref()
                .with_context(|| "Duration without an upper bound")?;

            let mut min: FAtom = factory.reify(min, Some(Span::instant(start)))?.try_into()?;
            let mut max: FAtom = factory.reify(max, Some(Span::instant(start)))?.try_into()?;

            if interval.is_left_open {
                min = min + FAtom::EPSILON;
            }
            if interval.is_right_open {
                max = max - FAtom::EPSILON;
            }

            let min = LinearSum::from(min);
            let max = LinearSum::from(max);

            factory
                .chronicle
                .constraints
                .push(Constraint::duration(Duration::Bounded { lb: min, ub: max }));
        }
    }

    let cost_expr = costs.costs.get(&action.name).or(costs.default.as_ref());
    if let Some(cost) = cost_expr {
        factory.set_cost(cost)?;
    }

    factory.build_template(action.name.clone())
}

fn read_method(container: Container, method: &up::Method, context: &mut Ctx) -> Result<ChronicleTemplate, Error> {
    let mut variables: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(container / VarType::Presence);
    variables.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, container / VarType::ChronicleStart);
    variables.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = if method.subtasks.is_empty() {
        start // no subtasks, the method is instantaneous
    } else {
        let end = context
            .model
            .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, container / VarType::ChronicleEnd);
        variables.push(end.into());
        end.into()
    };

    let mut name: Vec<SAtom> = Vec::with_capacity(1 + method.parameters.len());
    let base_name = &Sym::from(method.name.clone());
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

    let ch = Chronicle {
        kind: ChronicleKind::Method,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
        cost: None,
    };

    let mut factory = ChronicleFactory {
        context,
        chronicle: ch,
        container,
        parameters: Default::default(),
        variables,
    };

    // process the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for param in &method.parameters {
        factory.add_parameter(&param.name, &param.r#type)?;
    }

    let achieved_task = method
        .achieved_task
        .as_ref()
        .with_context(|| format!("Missing achieved task in method: {}", &method.name))?;
    let mut task_name = Vec::with_capacity(achieved_task.parameters.len() + 1);
    task_name.push(str_to_symbol(
        &achieved_task.task_name,
        &factory.context.model.shape.symbols,
    )?);
    for param in &achieved_task.parameters {
        let param = factory.reify(param, None)?;
        let param: SAtom = param.try_into()?;
        task_name.push(param);
    }
    factory.chronicle.task = Some(task_name);

    for subtask in &method.subtasks {
        factory.add_subtask(subtask)?;
    }

    for condition in &method.conditions {
        factory.add_condition(condition)?;
    }

    for constraint in &method.constraints {
        factory.enforce(constraint, None)?;
    }

    factory.build_template(method.name.clone())
}

fn kind(e: &Expression) -> Result<ExpressionKind, Error> {
    ExpressionKind::from_i32(e.kind).with_context(|| format!("Unknown expression kind id: {}", e.kind))
}

fn as_int(e: &Expression) -> Result<i32, Error> {
    if kind(e)? == ExpressionKind::Constant && e.r#type.starts_with("up:integer") {
        match e.atom.as_ref().unwrap().content.as_ref().unwrap() {
            Content::Int(i) => Ok(*i as i32),
            _ => bail!("Malformed message"),
        }
    } else {
        bail!("Expression is not a constant int")
    }
}
