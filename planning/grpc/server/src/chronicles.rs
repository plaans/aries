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
use unified_planning::{Assignment, TimedEffect};
use up::atom::Content;
use up::effect_expression::EffectKind;
use up::metric::MetricKind;
use up::timepoint::TimepointKind;
use up::{Expression, ExpressionKind, Problem};

/// Names for built in types. They contain UTF-8 symbols for sexiness
/// (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static FLUENT_TYPE: &str = "★fluent★";
static OBJECT_TYPE: &str = "★object★";

fn build_context(problem: &Problem) -> Result<Ctx, Error> {
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

        if let Some(scheduling_extension) = &problem.scheduling_extension {
            for activity in &scheduling_extension.activities {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(&activity.name),
                    tpe: Some(ACTION_TYPE.into()),
                });
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

    if let Some(min_eps) = &problem.epsilon {
        ensure!(
            min_eps.numerator == 1,
            "Only support epsilons with numerator equals to 1"
        );
        let scale: i32 = min_eps.denominator.try_into()?;
        TIME_SCALE.set(scale);
    }

    Ok(Ctx::new(Arc::new(symbol_table), state_variables))
}
pub fn problem_to_chronicles(problem: &Problem) -> Result<aries_planning::chronicles::Problem, Error> {
    let mut context = build_context(problem)?;

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

    if problem.has_feature(up::Feature::Scheduling) {
        // scheduling requires special handling
        return scheduling_problem_to_chronicles(context, init_ch, problem);
    }
    ensure!(problem.scheduling_extension.is_none());

    let mut factory = ChronicleFactory::new(&mut context, init_ch, Container::Base, vec![]);

    factory.add_initial_state(&problem.initial_state)?;
    factory.add_timed_effects(&problem.timed_effects)?;
    factory.add_goals(&problem.goals)?;

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

    Ok(aries_planning::chronicles::Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    })
}

fn scheduling_problem_to_chronicles(
    mut context: Ctx,
    init_ch: Chronicle,
    problem: &Problem,
) -> Result<aries_planning::chronicles::Problem, Error> {
    let mut instances = Vec::with_capacity(16);
    let mut factory = ChronicleFactory::new(&mut context, init_ch, Container::Base, vec![]);

    let Some(scheduling) = &problem.scheduling_extension else {
        bail!("Missing scheduling extension in problem");
    };
    // gather all variables
    for var in &scheduling.variables {
        factory.add_parameter(&var.name, &var.r#type)?;
    }
    for (act_id, activity) in scheduling.activities.iter().enumerate() {
        let act_id = act_id as u32;
        for var in &activity.parameters {
            factory.add_parameter(&var.name, &var.r#type)?;
        }

        let start = factory.create_timepoint(VarType::TaskStart(act_id));
        let end = {
            let duration = activity
                .duration
                .as_ref()
                .with_context(|| format!("Missing duration in durative action {}", activity.name))?;
            if let Some(dur) = get_fixed_duration(duration) {
                // a duration constraint is added later in the function for more complex durations
                start + dur
            } else {
                factory.create_timepoint(VarType::TaskEnd(act_id))
            }
        };
        factory.declare_time_interval(activity.name.clone(), start, end)?;
    }

    // an environonwment with all variable and timepoints definitions.
    // it must used as a context for all expression evaluations
    let global_env = factory.env.clone();

    factory.add_initial_state(&problem.initial_state)?;
    factory.add_goals(&problem.goals)?;

    for constraint in &scheduling.constraints {
        factory
            .enforce(
                constraint,
                Some(Span::interval(factory.chronicle.start, factory.chronicle.end)),
            )
            .with_context(|| format!("In problem constraint: {constraint}"))?;
    }

    instances.push(factory.build_instance(ChronicleOrigin::Original)?);

    for activity in &scheduling.activities {
        let cont = Container::Instance(instances.len());
        let chronicle = read_activity(cont, activity, &mut context, &global_env)?;
        instances.push(chronicle);
    }

    Ok(aries_planning::chronicles::Problem {
        context,
        templates: vec![],
        chronicles: instances,
    })
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

/// Structure that associate names to the corresonding variables.
#[derive(Clone, Default)]
struct Env {
    parameters: HashMap<String, Variable>,
    intervals: HashMap<String, (Time, Time)>,
}

struct ChronicleFactory<'a> {
    context: &'a mut Ctx,
    chronicle: Chronicle,
    container: Container,
    variables: Vec<Variable>,
    env: Env,
}

impl<'a> ChronicleFactory<'a> {
    pub fn new(context: &'a mut Ctx, chronicle: Chronicle, container: Container, variables: Vec<Variable>) -> Self {
        ChronicleFactory {
            context,
            chronicle,
            container,
            variables,
            env: Default::default(),
        }
    }

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
            .env
            .parameters
            .get(name)
            .with_context(|| format!("Unknown parameter: {name}"))?;
        Ok(var.into())
    }

    /// Adds a parameter to the chronicle name. This creates a new variable to with teh corresponding type.
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
        assert!(!self.env.parameters.contains_key(&name_string));
        self.env.parameters.insert(name_string, arg.into());

        Ok(arg)
    }

    fn declare_time_interval(&mut self, name: String, start: FAtom, end: FAtom) -> Result<(), Error> {
        ensure!(
            !self.env.intervals.contains_key(&name),
            "A container named {name} already exists"
        );
        self.env.intervals.insert(name, (start, end));
        Ok(())
    }

    fn create_timepoint(&mut self, vartype: VarType) -> FAtom {
        let tp = self.context.model.new_optional_fvar(
            0,
            INT_CST_MAX,
            TIME_SCALE.get(),
            self.chronicle.presence,
            self.container / vartype,
        );
        self.variables.push(tp.into());
        FAtom::from(tp)
    }

    fn add_up_effect(&mut self, eff: &up::Effect) -> Result<(), Error> {
        let effect_span = if let Some(occurrence) = &eff.occurrence_time {
            let start = self.read_timing(occurrence)?;
            Span::interval(start, start + FAtom::EPSILON)
        } else {
            ensure!(
                self.chronicle.start == self.chronicle.end,
                "Durative action with untimed effect."
            );
            Span::interval(self.chronicle.end, self.chronicle.end + Time::EPSILON)
        };
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
        self.add_effect(effect_span, sv, value, effect_kind)
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
        let operation = match kind {
            EffectKind::Assign => EffectOp::Assign(value),
            EffectKind::Increase => {
                let value = IAtom::try_from(value).context("Increase effect require an integer value.")?;
                EffectOp::Increase(LinearSum::from(value))
            }
            EffectKind::Decrease => {
                let value = IAtom::try_from(value).context("Decrease effect require an integer value.")?;
                EffectOp::Increase(-LinearSum::from(value))
            }
        };
        self.chronicle.effects.push(Effect {
            transition_start: span.start,
            transition_end: span.end,
            min_mutex_end: Vec::new(),
            state_var: sv,
            operation,
        });
        Ok(())
    }

    /// Converts initial state to a set of effects at the start time
    fn add_initial_state(&mut self, init_state: &[Assignment]) -> Result<(), Error> {
        for assignment in init_state {
            let state_var = assignment
                .fluent
                .as_ref()
                .context("Initial state assignment has no valid fluent")?;
            let value = assignment
                .value
                .as_ref()
                .context("Initial state assignment has no valid value")?;
            let init_time = Span::instant(self.chronicle.start);

            self.add_effect(init_time, state_var, value, EffectKind::Assign)?;
        }
        Ok(())
    }

    fn add_timed_effects(&mut self, timed_effects: &[TimedEffect]) -> Result<(), Error> {
        for timed_eff in timed_effects {
            let at = timed_eff
                .occurrence_time
                .as_ref()
                .context("Missing time on timed-effect")?;
            let at = self.read_timing(at)?;
            let span = Span::interval(at, at + FAtom::EPSILON);
            let eff = timed_eff.effect.as_ref().context("Missing effect in timed-effect")?;

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
            self.add_effect(span, sv, value, effect_kind)?;
        }
        Ok(())
    }

    /// Goals are translated to conditions at the chronicle end time
    fn add_goals(&mut self, goals: &[up::Goal]) -> Result<(), Error> {
        for goal in goals {
            let span = if let Some(itv) = &goal.timing {
                self.read_time_interval(itv)
                    .with_context(|| format!("In time interval of goal: {goal:?}"))?
            } else {
                Span::instant(self.chronicle.end)
            };
            if let Some(goal) = &goal.goal {
                self.enforce(goal, Some(span))
                    .with_context(|| format!("In goal expression {goal}",))?;
            }
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
        if let Some(cond) = &condition.cond {
            let span = if let Some(itv) = &condition.span {
                self.read_time_interval(itv)?
            } else {
                Span::instant(self.chronicle.start)
            };
            self.enforce(cond, Some(span))?;
        }
        Ok(())
    }

    fn set_duration(&mut self, duration: &up::Duration) -> Result<(), Error> {
        let start = self.chronicle.start;
        if let Some(interval) = duration.controllable_in_bounds.as_ref() {
            let min = interval
                .lower
                .as_ref()
                .with_context(|| "Duration without a lower bound")?;
            let max = interval
                .upper
                .as_ref()
                .with_context(|| "Duration without an upper bound")?;

            let mut min: FAtom = self.reify(min, Some(Span::instant(start)))?.try_into()?;
            let mut max: FAtom = self.reify(max, Some(Span::instant(start)))?.try_into()?;

            if interval.is_left_open {
                min = min + FAtom::EPSILON;
            }
            if interval.is_right_open {
                max = max - FAtom::EPSILON;
            }

            let min = LinearSum::from(min);
            let max = LinearSum::from(max);

            self.chronicle
                .constraints
                .push(Constraint::duration(Duration::Bounded { lb: min, ub: max }));
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
        let task_name = task_name.iter().map(|satom| Atom::Sym(*satom)).collect();
        self.chronicle.subtasks.push(SubTask {
            id: Some(subtask.id.clone()),
            start,
            end,
            task_name,
        });
        self.declare_time_interval(subtask.id.clone(), start, end)?;
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
                    "up:lt" | "up:le" => {
                        ensure!(params.len() == 2, "`=` operator should have exactly 2 arguments");
                        let params: Vec<Atom> = params
                            .iter()
                            .map(|param| self.reify(param, span))
                            .collect::<Result<Vec<_>, _>>()?;

                        let value = Lit::try_from(value)?;
                        let constraint = match operator {
                            "up:lt" => Constraint::reified_lt(params[0], params[1], value),
                            "up:le" => Constraint::reified_leq(params[0], params[1], value),
                            _ => unreachable!(),
                        };
                        self.chronicle.constraints.push(constraint);
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
                        let interval = self
                            .env
                            .intervals
                            .get(container)
                            .with_context(|| format!("Unknown interval {container}"))?;

                        match operator {
                            "up:start" => interval.0,
                            "up:end" => interval.1,
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
                        "up:lt" | "up:le" => {
                            ensure!(params.len() == 2, "`<` operator should have exactly 2 arguments");
                            let value = self.create_bool_variable(VarType::Reification);
                            let tpe = match operator {
                                "up:lt" => ConstraintType::Lt,
                                "up:le" => ConstraintType::Leq,
                                _ => unreachable!(),
                            };
                            let constraint = Constraint {
                                variables: params,
                                tpe,
                                value: Some(value),
                            };
                            self.chronicle.constraints.push(constraint);
                            Ok(value.into())
                        }
                        "up:plus" => {
                            let value: IVar = self
                                .create_variable(Type::UNBOUNDED_INT, VarType::Reification)
                                .try_into()?;
                            let mut sum = -LinearSum::from(value);
                            for param in params {
                                sum += LinearSum::try_from(param)?;
                            }
                            self.chronicle.constraints.push(Constraint::linear_eq_zero(sum));
                            Ok(value.into())
                        }
                        "up:minus" => {
                            ensure!(params.len() == 2, "`-` operator should have exactly 2 arguments");
                            let value: IVar = self
                                .create_variable(Type::UNBOUNDED_INT, VarType::Reification)
                                .try_into()?;
                            let sum = LinearSum::try_from(params[0])?
                                - LinearSum::try_from(params[1])?
                                - LinearSum::from(value);
                            self.chronicle.constraints.push(Constraint::linear_eq_zero(sum));
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
            ensure!(TIME_SCALE.get() % denom == 0, "Time scale beyond what is supported.");
            let scale = TIME_SCALE.get() / denom;
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
fn get_fixed_duration(duration: &up::Duration) -> Option<IntCst> {
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

    let start = context.model.new_optional_fvar(
        0,
        INT_CST_MAX,
        TIME_SCALE.get(),
        prez,
        container / VarType::ChronicleStart,
    );
    variables.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = match action_kind {
        ChronicleKind::Problem | ChronicleKind::Method => unreachable!(),
        ChronicleKind::DurativeAction => {
            let duration = action
                .duration
                .as_ref()
                .with_context(|| format!("Missing duration in durative action {}", action.name))?;
            if let Some(dur) = get_fixed_duration(duration) {
                // a duration constraint is added later in the function for more complex durations
                start + dur
            } else {
                let end = context.model.new_optional_fvar(
                    0,
                    INT_CST_MAX,
                    TIME_SCALE.get(),
                    prez,
                    container / VarType::ChronicleEnd,
                );
                variables.push(end.into());
                end.into()
            }
        }
        ChronicleKind::Action => start, // non-temporal actions are instantaneous
    };

    let mut name: Vec<Atom> = Vec::with_capacity(1 + action.parameters.len());
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

    let mut factory = ChronicleFactory::new(context, ch, container, variables);

    // process the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for param in &action.parameters {
        factory.add_parameter(&param.name, &param.r#type)?;
    }

    // set the action's achieved task to be the same as the its name
    // note that we wait until all parameters have been add to the name before doing this
    factory.chronicle.task = Some(factory.chronicle.name.clone());

    // Process the effects of the action
    for eff in &action.effects {
        factory.add_up_effect(eff)?;
    }

    for condition in &action.conditions {
        factory.add_condition(condition)?;
    }

    if let Some(duration) = action.duration.as_ref() {
        factory.set_duration(duration)?;
    }

    let cost_expr = costs.costs.get(&action.name).or(costs.default.as_ref());
    if let Some(cost) = cost_expr {
        factory.set_cost(cost)?;
    }

    factory.build_template(action.name.clone())
}

fn read_activity(
    container: Container,
    activity: &up::Activity,
    context: &mut Ctx,
    global_env: &Env,
) -> Result<ChronicleInstance, Error> {
    // similar to an action but all variables have been previously declared in the global_env
    let (start, end) = global_env.intervals[&activity.name];

    let mut name: Vec<Atom> = Vec::with_capacity(1 + activity.parameters.len());
    let base_name = &Sym::from(activity.name.clone());
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
    for param in &activity.parameters {
        name.push(global_env.parameters[&param.name].into());
    }

    let ch = Chronicle {
        kind: ChronicleKind::DurativeAction,
        presence: Lit::TRUE,
        start,
        end,
        name,
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
        cost: None,
    };

    let mut factory = ChronicleFactory::new(context, ch, container, vec![]);
    factory.env = global_env.clone();

    // set the action's achieved task to be the same as the its name
    // note that we wait until all parameters have been add to the name before doing this
    factory.chronicle.task = None;

    // // Process the effects of the action
    for eff in &activity.effects {
        factory.add_up_effect(eff)?;
    }
    //
    for condition in &activity.conditions {
        factory.add_condition(condition)?;
    }
    for constraint in &activity.constraints {
        factory.enforce(constraint, Some(Span::interval(start, end)))?;
    }

    if let Some(duration) = activity.duration.as_ref() {
        factory.set_duration(duration)?;
    }

    factory.build_instance(ChronicleOrigin::Original)
}

fn read_method(container: Container, method: &up::Method, context: &mut Ctx) -> Result<ChronicleTemplate, Error> {
    let mut variables: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(container / VarType::Presence);
    variables.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context.model.new_optional_fvar(
        0,
        INT_CST_MAX,
        TIME_SCALE.get(),
        prez,
        container / VarType::ChronicleStart,
    );
    variables.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = if method.subtasks.is_empty() {
        start // no subtasks, the method is instantaneous
    } else {
        let end = context.model.new_optional_fvar(
            0,
            INT_CST_MAX,
            TIME_SCALE.get(),
            prez,
            container / VarType::ChronicleEnd,
        );
        variables.push(end.into());
        end.into()
    };

    let mut name: Vec<Atom> = Vec::with_capacity(1 + method.parameters.len());
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

    let mut factory = ChronicleFactory::new(context, ch, container, variables);

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
    let task_name = task_name.iter().map(|satom| Atom::from(*satom)).collect();
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
