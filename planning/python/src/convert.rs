//! Conversion of a unified-planning problem (as Python objects) into chronicles.
//!
//! This is a port of `planning/grpc/server/src/chronicles.rs`, with the same functions in the same order.
//! Protobuf messages are replaced by the corresponding Python objects, read through PyO3.
//! Expressions are first converted to a protobuf-like `up::Expression` so that their handling is unchanged.

use crate::up;
use crate::up::atom::Content;
use crate::up::{Assignment, Expression, ExpressionKind, Metric, MetricKind};
use anyhow::{anyhow, bail, ensure, Context, Error};
use aries_env_param::EnvParam;
use aries_planning::chronicles::constraints::{Constraint, ConstraintType, Duration};
use aries_planning::chronicles::VarType::Reification;
use aries_planning::chronicles::*;
use aries_planning::legacy::input::Sym;
use aries_planning::legacy::utils::enumerate;
use aries_planning::legacy::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_solver::core::{IntCst, Lit, INT_CST_MAX, INT_CST_MIN};
use aries_solver::lang::*;
use itertools::Itertools;
use num_rational::Ratio;
use pyo3::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

type Py<'py> = Bound<'py, PyAny>;

/// If true, Aries will assume that all real state variables are ints.
/// This is typically useful when the input problem was parsed from PDDL that does not
/// distinguishes between ints and reals.
static ASSUME_REALS_ARE_INTS: EnvParam<bool> = EnvParam::new("ARIES_UP_ASSUME_REALS_ARE_INTS", "false");

/// Names for built in types. They contain UTF-8 symbols for sexiness
/// (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static FLUENT_TYPE: &str = "★fluent★";
static OBJECT_TYPE: &str = "★object★";

fn from_upf_type(name: &str, types: &TypeHierarchy) -> anyhow::Result<Type> {
    let int_type_regex = Regex::new(r"^up:integer\[(-?\d+)\s*,\s*(-?\d+)\]$").unwrap();
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
    } else if name == "up:real" && ASSUME_REALS_ARE_INTS.get() {
        Ok(Type::UNBOUNDED_INT)
    } else if name.starts_with("up:real") {
        Err(anyhow!("Real types are not supported"))
    } else if let Some(tpe) = types.id_of(name) {
        Ok(Type::Sym(tpe))
    } else {
        Err(anyhow!("Unsupported type `{}`", name))
    }
}

fn is_hierarchical(problem: &Py) -> anyhow::Result<bool> {
    Ok(problem.hasattr("task_network")?)
}

fn is_scheduling(problem: &Py) -> anyhow::Result<bool> {
    Ok(problem.hasattr("activities")?)
}

fn build_context(problem: &Py) -> Result<Ctx, Error> {
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
        for tpe in up::list(&problem.getattr("user_types")?)? {
            let father = tpe.getattr("father")?;
            let parent = if !father.is_none() {
                Some(Sym::from(up::proto_type(&father)?))
            } else {
                None
            };
            let type_name = Sym::from(up::proto_type(&tpe)?);
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
        for obj in up::list(&problem.getattr("all_objects")?)? {
            let object_symbol = Sym::from(up::name(&obj)?);
            let object_type = Sym::from(up::proto_type(&obj.getattr("type")?)?);

            // declare the object as a new symbol with the given type
            symbols.push(TypedSymbol {
                symbol: object_symbol,
                tpe: Some(object_type),
            });
        }

        // record all symbols representing fluents
        for fluent in up::list(&problem.getattr("fluents")?)? {
            symbols.push(TypedSymbol {
                symbol: Sym::from(up::name(&fluent)?),
                tpe: Some(FLUENT_TYPE.into()),
            });
        }

        // actions are symbols as well, add them to the table
        if problem.hasattr("actions")? {
            for action in up::list(&problem.getattr("actions")?)? {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(up::name(&action)?),
                    tpe: Some(ACTION_TYPE.into()),
                });
            }
        }

        if is_hierarchical(problem)? {
            for task in up::list(&problem.getattr("tasks")?)? {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(up::name(&task)?),
                    tpe: Some(ABSTRACT_TASK_TYPE.into()),
                })
            }

            for method in up::list(&problem.getattr("methods")?)? {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(up::name(&method)?),
                    tpe: Some(METHOD_TYPE.into()),
                })
            }
        }

        if is_scheduling(problem)? {
            for activity in up::list(&problem.getattr("activities")?)? {
                symbols.push(TypedSymbol {
                    symbol: Sym::from(up::name(&activity)?),
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

    let mut state_variables = vec![];
    {
        for fluent in up::list(&problem.getattr("fluents")?)? {
            let fluent_name = up::name(&fluent)?;
            let name = Sym::from(fluent_name.clone());
            let sym = symbol_table
                .id(&name)
                .with_context(|| format!("Fluent `{}` not found in symbol table", fluent_name))?;
            let parameters = up::list(&fluent.getattr("signature")?)?;
            let mut signature = Vec::with_capacity(1 + parameters.len());

            for arg in &parameters {
                let arg_type = up::proto_type(&arg.getattr("type")?)?;
                signature.push(from_upf_type(arg_type.as_str(), &types).with_context(|| {
                    format!(
                        "Invalid parameter type `{}` for fluent parameter `{}`",
                        arg_type,
                        up::name(arg).unwrap_or_default()
                    )
                })?);
            }

            signature.push(from_upf_type(&up::proto_type(&fluent.getattr("type")?)?, &types)?);

            state_variables.push(Fluent { name, sym, signature });
        }
    }

    let epsilon = problem.getattr("epsilon")?;
    if !epsilon.is_none() {
        let min_eps: Ratio<i64> = epsilon.extract()?;
        ensure!(
            *min_eps.numer() == 1,
            "Only support epsilons with numerator equals to 1"
        );
        let scale: IntCst = (*min_eps.denom()).try_into()?;
        TIME_SCALE.set(scale);
    } else {
        // the scale may have been set by a previous problem of the same process
        TIME_SCALE.reset();
    }

    Ok(Ctx::new(Arc::new(symbol_table), state_variables))
}

pub fn problem_to_chronicles(problem: &Py) -> Result<aries_planning::chronicles::Problem, Error> {
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

    if up::has_feature(problem, "SCHEDULING")? {
        // scheduling requires special handling
        return scheduling_problem_to_chronicles(context, init_ch, problem);
    }
    ensure!(!is_scheduling(problem)?);

    let mut factory = ChronicleFactory::new(&mut context, init_ch, Container::Base, vec![]);

    let initial_state = up::read_assignments(&problem.getattr("explicit_initial_values")?)?;
    let fluents = up::read_fluents(problem)?;
    let metrics = up::read_metrics(problem)?;
    factory.add_initial_state(&initial_state, &fluents)?;
    factory.add_timed_effects(&up::dict_of_lists(&problem.getattr("timed_effects")?)?)?;
    factory.add_goals(&read_goals(problem)?)?;
    factory.add_final_value_metric(&metrics)?;

    if is_hierarchical(problem)? {
        let tn = problem.getattr("task_network")?;
        for var in up::list(&tn.getattr("variables")?)? {
            factory.add_parameter(up::name(&var)?, up::proto_type(&var.getattr("type")?)?)?;
        }

        for subtask in up::list(&tn.getattr("subtasks")?)? {
            factory.add_subtask(&subtask).with_context(|| {
                format!(
                    "Adding initial task {} ({})",
                    subtask_task_name(&subtask).unwrap_or_default(),
                    subtask_id(&subtask).unwrap_or_default()
                )
            })?;
        }

        for constraint in up::list(&tn.getattr("constraints")?)? {
            factory
                .enforce(&up::read_expression(&constraint)?, None)
                .with_context(|| format!("In initial task network constraint: {constraint}"))?;
        }
    }

    let init_ch = factory.build_instance(ChronicleOrigin::Original)?;

    ensure!(metrics.len() <= 1, "No support for multiple metrics.");
    let action_costs = metrics
        .iter()
        .find(|metric| metric.kind == MetricKind::MinimizeActionCosts);
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

    for a in up::list(&problem.getattr("actions")?)? {
        let cont = Container::Template(templates.len());
        let template = read_action(cont, &a, &action_costs, &mut context)
            .with_context(|| format!("Adding action: {}", up::name(&a).unwrap_or_default()))?;
        templates.push(template);
    }

    if is_hierarchical(problem)? {
        for method in up::list(&problem.getattr("methods")?)? {
            let cont = Container::Template(templates.len());
            let template = read_method(cont, &method, &mut context)
                .with_context(|| format!("Adding method: {}", up::name(&method).unwrap_or_default()))?;
            templates.push(template);
        }
    }

    Ok(aries_planning::chronicles::Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    })
}

/// Goals of the problem, with an optional time interval.
fn read_goals<'py>(problem: &Py<'py>) -> anyhow::Result<Vec<(Option<Py<'py>>, Py<'py>)>> {
    let mut goals: Vec<_> = up::list(&problem.getattr("goals")?)?
        .into_iter()
        .map(|g| (None, g))
        .collect();
    for (timing, goal) in up::dict_of_lists(&problem.getattr("timed_goals")?)? {
        goals.push((Some(timing), goal));
    }
    Ok(goals)
}

fn scheduling_problem_to_chronicles(
    mut context: Ctx,
    init_ch: Chronicle,
    problem: &Py,
) -> Result<aries_planning::chronicles::Problem, Error> {
    let mut instances = Vec::with_capacity(16);
    let mut factory = ChronicleFactory::new(&mut context, init_ch, Container::Base, vec![]);

    ensure!(is_scheduling(problem)?, "Missing scheduling extension in problem");
    let activities = up::list(&problem.getattr("activities")?)?;
    // gather all variables
    for var in up::list(&problem.getattr("base_variables")?)? {
        factory.add_parameter(up::name(&var)?, up::proto_type(&var.getattr("type")?)?)?;
    }
    for (act_id, activity) in activities.iter().enumerate() {
        let activity_name = up::name(activity)?;
        let prez = if activity.getattr("optional")?.is_truthy()? {
            factory.create_presence_literal()
        } else {
            Lit::TRUE
        };
        factory.env.presences.insert(activity_name.clone(), prez);
        let act_id = act_id as u32;
        for var in up::list(&activity.getattr("parameters")?)? {
            factory.add_optional_parameter(up::name(&var)?, up::proto_type(&var.getattr("type")?)?, prez)?;
        }

        let start = factory.create_optional_timepoint(VarType::TaskStart(act_id), prez);
        let end = {
            let duration = activity
                .getattr("duration")
                .with_context(|| format!("Missing duration in durative action {}", activity_name))?;
            if let Some(dur) = get_fixed_duration(&duration)? {
                // a duration constraint is added later in the function for more complex durations
                start + dur
            } else {
                factory.create_optional_timepoint(VarType::TaskEnd(act_id), prez)
            }
        };
        factory.declare_time_interval(activity_name, start, end)?;
    }

    // an environonwment with all variable and timepoints definitions.
    // it must used as a context for all expression evaluations
    let global_env = factory.env.clone();

    let initial_state = up::read_assignments(&problem.getattr("initial_values")?)?;
    let fluents = up::read_fluents(problem)?;
    let mut timed_effects = Vec::new();
    for item in up::list(&problem.getattr("base_effects")?)? {
        timed_effects.push(item.extract::<(Py, Py)>()?);
    }
    let mut goals = Vec::new();
    for item in up::list(&problem.getattr("base_conditions")?)? {
        let (timing, goal) = item.extract::<(Py, Py)>()?;
        goals.push((Some(timing), goal));
    }
    factory.add_initial_state(&initial_state, &fluents)?;
    factory.add_timed_effects(&timed_effects)?;
    factory.add_goals(&goals)?;

    let mut scoped_constraints = vec![];
    for scoped in up::list(&problem.getattr("base_scoped_constraints")?)? {
        let (constraint, scope) = scoped.extract::<(Py, Py)>()?;
        let constraint = up::read_expression(&constraint)?;
        let mut scope_lits = vec![];
        for scope_elem in up::list(&scope)? {
            let atom = factory
                .reify(&up::read_expression(&scope_elem)?, None)
                .with_context(|| format!("Unable to reify scope element of constraint: {scoped:?}"))?;
            let Atom::Bool(lit) = atom else {
                bail!("scoped element was not a boolean")
            };
            scope_lits.push(lit);
        }
        let scope = factory.context.model.get_conjunctive_scope(&scope_lits);
        scoped_constraints.push((constraint, scope));
    }

    instances.push(factory.build_instance(ChronicleOrigin::Original)?);

    for activity in &activities {
        let cont = Container::Instance(instances.len());
        let chronicle = read_activity(cont, activity, &mut context, &global_env)?;
        instances.push(chronicle);
    }
    scoped_constraints.sort_by_key(|(_e, scope)| *scope);
    use itertools::Itertools;
    let grouped_by_scope = scoped_constraints.iter().chunk_by(|(_e, s)| *s);
    for (scope, scoped_constraints) in &grouped_by_scope {
        let constraints = scoped_constraints.map(|(e, _scope)| e).collect_vec();
        let ch = standalone_chronicle_for_constraints(scope, &constraints, &mut context, &global_env)?;

        instances.push(ch);
    }

    Ok(aries_planning::chronicles::Problem {
        context,
        templates: vec![],
        chronicles: instances,
    })
}

fn standalone_chronicle_for_constraints(
    scope: Lit,
    constraints: &[&Expression],
    context: &mut Ctx,
    global_env: &Env,
) -> Result<ChronicleInstance, Error> {
    let container = Container::Base;

    let ch = Chronicle {
        kind: ChronicleKind::Problem,
        presence: scope,
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

    let mut factory = ChronicleFactory::new(context, ch, container, vec![]);
    factory.env = global_env.clone();

    for constraint in constraints {
        factory.enforce(constraint, None)?;
    }

    factory.build_instance(ChronicleOrigin::Original)
}

struct ActionCosts {
    costs: HashMap<String, Expression>,
    default: Option<Expression>,
}

fn atom_to_symid(atom: &up::Atom, symbols: &SymbolTable) -> anyhow::Result<SymId> {
    let Some(Content::Symbol(s)) = &atom.content else {
        bail!("No symbolic content in atom: {atom:?}");
    };
    symbols.id(s).with_context(|| format!("Unknown symbol: {s:?}"))
}

fn str_to_symbol(name: &str, symbol_table: &SymbolTable) -> anyhow::Result<SAtom> {
    let sym = symbol_table
        .id(name)
        .with_context(|| format!("Unknown symbol / operator `{name}`"))?;
    let tpe = symbol_table.type_of(sym);
    Ok(SAtom::new_constant(sym, tpe))
}

fn read_atom(atom: &up::Atom, symbol_table: &SymbolTable) -> Result<Atom, Error> {
    if let Some(atom_content) = atom.content.clone() {
        match atom_content {
            up::atom::Content::Symbol(s) => {
                let atom = str_to_symbol(s.as_str(), symbol_table)?;
                Ok(atom.into())
            }
            up::atom::Content::Int(i) => Ok(Atom::from(i as IntCst)),
            up::atom::Content::Real(_f) => {
                bail!("`Real` type not supported yet")
            }
            up::atom::Content::Boolean(b) => Ok(Atom::Bool(b.into())),
        }
    } else {
        bail!("Unsupported atom")
    }
}

struct InitFact {
    fluent: SymId,
    params: Vec<SymId>,
    value: Atom,
}

fn read_init_fact(fact: &up::Assignment, symbols: &SymbolTable) -> anyhow::Result<InitFact> {
    let get_atom = |e: &up::Expression| {
        let atom = e.atom.as_ref().with_context(|| format!("Not an atom: {e:?}"))?;
        read_atom(atom, symbols)
    };
    let get_sym = |e: &up::Expression| {
        let atom = e.atom.as_ref().with_context(|| format!("Not an atom: {e:?}"))?;
        match atom.content.as_ref() {
            Some(up::atom::Content::Symbol(s)) => symbols.id(s).with_context(|| format!("Unknown symbol: {:?}", s)),
            _ => bail!("Not a symbol {:?}", e),
        }
    };
    let Some(fluent) = fact.fluent.as_ref() else {
        bail!("Malformed init fact: {fact:?}")
    };

    // first symbol as the fluent, the rest are the parameters
    ensure!(!fluent.list.is_empty(), "Malformed fluent expression: {fluent:?}");
    let fluent_sym = atom_to_symid(
        fluent.list[0]
            .atom
            .as_ref()
            .with_context(|| format!("Missing fluent atom : {:?}", fluent))?,
        symbols,
    )?;
    let params: Vec<SymId> = fluent.list[1..].iter().map(get_sym).try_collect()?;

    let Some(value) = fact.value.as_ref() else {
        bail!("Malformed init fact: {fact:?}")
    };
    let value = get_atom(value)?;
    Ok(InitFact {
        fluent: fluent_sym,
        params,
        value,
    })
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
    presences: HashMap<String, Lit>,
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
            label: ChronicleLabel::Action(label),
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
    fn add_parameter(&mut self, name: impl Into<Sym>, tpe: impl Into<Sym>) -> Result<Variable, Error> {
        self.add_optional_parameter(name, tpe, self.chronicle.presence)
    }
    /// same but with a presence literal that may differ from the one of the chronicle
    fn add_optional_parameter(
        &mut self,
        name: impl Into<Sym>,
        tpe: impl Into<Sym>,
        presence: Lit,
    ) -> Result<Variable, Error> {
        let name = name.into();

        let tpe = tpe.into();
        let tpe = from_upf_type(tpe.as_ref(), &self.context.get_symbol_table().types)
            .with_context(|| format!("Unknown argument type: {tpe}"))?;
        let label = self.container / VarType::Parameter(name.to_string());
        let arg: Variable = match tpe {
            Type::Sym(tpe) => self.context.new_optional_sym_var(tpe, presence, label).into(),
            Type::Int { lb, ub } => self.context.model.new_optional_ivar(lb, ub, presence, label).into(),
            Type::Fixed(denom) => self
                .context
                .new_optional_fvar(INT_CST_MIN, INT_CST_MAX, denom, presence, label)
                .into(),
            Type::Bool => self.context.model.new_optional_bvar(presence, label).into(),
        };

        // append parameters to the name of the chronicle
        self.chronicle.name.push(arg.into());

        self.variables.push(arg);

        // add parameter to the mapping
        let name_string = name.to_string();
        assert!(!self.env.parameters.contains_key(&name_string));
        self.env.parameters.insert(name_string, arg);

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
        self.create_optional_timepoint(vartype, self.chronicle.presence)
    }
    fn create_optional_timepoint(&mut self, vartype: VarType, presence: Lit) -> FAtom {
        let tp = self
            .context
            .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE.get(), presence, self.container / vartype);
        self.variables.push(tp.into());
        FAtom::from(tp)
    }

    fn create_presence_literal(&mut self) -> Lit {
        let bvar = self
            .context
            .model
            .new_presence_variable(Lit::TRUE, self.container / VarType::Presence);
        self.variables.push(bvar.into());
        bvar.true_lit()
    }

    /// Adds an effect, with its occurrence time if it belongs to a durative action.
    fn add_up_effect(&mut self, eff: &Py, occurrence_time: Option<&Py>) -> Result<(), Error> {
        let effect_span = if let Some(occurrence) = occurrence_time {
            let start = self.read_timing(occurrence)?;
            Span::interval(start, start + FAtom::EPSILON)
        } else {
            ensure!(
                self.chronicle.start == self.chronicle.end,
                "Durative action with untimed effect."
            );
            Span::interval(self.chronicle.end, self.chronicle.end + Time::EPSILON)
        };
        let sv = up::read_expression(&eff.getattr("fluent")?)?;
        let value = up::read_expression(&eff.getattr("value")?)?;
        let effect_kind = effect_kind(eff)?;
        self.add_effect(effect_span, &sv, &value, effect_kind)
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
                let value = VarCst::try_from(value).context("Increase effect require an integer value.")?;
                EffectOp::Increase(LinearSum::from(value))
            }
            EffectKind::Decrease => {
                let value = VarCst::try_from(value).context("Decrease effect require an integer value.")?;
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
    fn add_initial_state(&mut self, init_state: &[Assignment], fluents: &[up::Fluent]) -> Result<(), Error> {
        let mut explicit_init_facts = Vec::with_capacity(init_state.len());
        for init_assignment in init_state {
            explicit_init_facts.push(read_init_fact(init_assignment, &self.context.symbols)?)
        }

        for up_fluent in fluents {
            let symbols = self.context.get_symbol_table();
            let fluent_sym = symbols
                .id(&up_fluent.name)
                .with_context(|| format!("Unknown symbol: {}", up_fluent.name))?;
            let fluent = self
                .context
                .fluents
                .iter()
                .find(|f| f.sym == fluent_sym)
                .with_context(|| format!("No such fluent: {}", up_fluent.name))?
                .clone();

            if let Some(default) = &up_fluent.default_value {
                // fluent has a default value
                let default = read_atom(
                    default
                        .atom
                        .as_ref()
                        .with_context(|| format!("Not an atom in default fluent value: {:?}", up_fluent))?,
                    symbols,
                )?;

                // gather all values that are explicitly set for this fluent
                let mut explicit_values: HashMap<&[SymId], Atom> = Default::default();
                for fact in &explicit_init_facts {
                    if fact.fluent == fluent_sym {
                        explicit_values.insert(&fact.params, fact.value);
                    }
                }

                // build an iterator over all combinations of values for the fluent's parameters
                let arg_types: Vec<TypeId> = fluent
                    .argument_types()
                    .iter()
                    .map(|tpe| TypeId::try_from(*tpe))
                    .try_collect()
                    .context("unsupported non symbolic type in fluent parameters")?;
                let arg_domains = arg_types.iter().map(|tpe| symbols.instances_of_type(*tpe)).collect();
                use aries_planning::legacy::utils::StreamingIterator;
                let mut combinations = enumerate(arg_domains);

                // for each possible instantiation of the fluent add an initial fact (with a default)
                while let Some(comb) = combinations.next() {
                    let value = explicit_values.get(comb).unwrap_or(&default);
                    self.add_init_fact(fluent.clone(), comb, *value)?;
                }
            } else {
                // no default value for fluent, only put the value explicitly stated
                for explicit_fact in &explicit_init_facts {
                    if explicit_fact.fluent == fluent_sym {
                        self.add_init_fact(fluent.clone(), &explicit_fact.params, explicit_fact.value)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn add_init_fact(&mut self, fluent: Arc<Fluent>, args: &[SymId], value: Atom) -> anyhow::Result<()> {
        let args = args
            .iter()
            .map(|&sym| SAtom::new_constant(sym, self.context.get_symbol_table().type_of(sym)))
            .collect();
        let sv = StateVar { fluent, args };

        self.chronicle.effects.push(Effect {
            transition_start: self.context.origin(),
            transition_end: self.context.origin(),
            min_mutex_end: vec![],
            state_var: sv,
            operation: EffectOp::Assign(value),
        });
        Ok(())
    }

    /// Adds the given `(timing, effect)` pairs.
    fn add_timed_effects(&mut self, timed_effects: &[(Py, Py)]) -> Result<(), Error> {
        for (at, eff) in timed_effects {
            let at = self.read_timing(at)?;
            let span = Span::interval(at, at + FAtom::EPSILON);

            let sv = up::read_expression(&eff.getattr("fluent")?)
                .with_context(|| format!("Effect expression has no fluent: {eff:?}"))?;

            let value =
                up::read_expression(&eff.getattr("value")?).with_context(|| format!("Effect has no value: {eff:?}"))?;

            let effect_kind = effect_kind(eff)?;
            self.add_effect(span, &sv, &value, effect_kind)?;
        }
        Ok(())
    }

    /// Goals are translated to conditions at the chronicle end time
    fn add_goals(&mut self, goals: &[(Option<Py>, Py)]) -> Result<(), Error> {
        for (timing, goal) in goals {
            let span = if let Some(itv) = timing {
                self.read_time_interval(itv)
                    .with_context(|| format!("In time interval of goal: {goal:?}"))?
            } else {
                Span::instant(self.chronicle.end)
            };
            self.enforce(&up::read_expression(goal)?, Some(span))
                .with_context(|| format!("In goal expression {goal}",))?;
        }
        Ok(())
    }

    /// Final value to minimize is converted to a condition at the chronicle end time
    fn add_final_value_metric(&mut self, metrics: &[Metric]) -> Result<(), Error> {
        ensure!(metrics.len() <= 1, "Unsupported: multiple metrics provided.");
        let Some(metric) = metrics.first() else {
            return Ok(()); // no metric to handle
        };
        if let MetricKind::MinimizeExpressionOnFinalState | MetricKind::MaximizeExpressionOnFinalState = metric.kind {
            let expr = metric
                .expression
                .as_ref()
                .context("Trying to optimize an empty expression metric.")?;
            let value = self.reify(expr, Some(Span::instant(self.context.horizon())))?;
            self.context.set_metric_final_value(value.try_into()?);
        };
        Ok(())
    }

    fn create_variable(&mut self, tpe: Type, var_type: VarType) -> Variable {
        let var: Variable = match tpe {
            Type::Sym(tpe) => self
                .context
                .new_optional_sym_var(tpe, self.chronicle.presence, self.container / var_type)
                .into(),
            Type::Int { lb, ub } => self
                .context
                .model
                .new_optional_ivar(lb, ub, self.chronicle.presence, self.container / var_type)
                .into(),
            Type::Fixed(denom) => self
                .context
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

    /// Adds a condition, holding over the given time interval (or at the chronicle start if none).
    fn add_condition(&mut self, cond: &Py, span: Option<&Py>) -> Result<(), Error> {
        let span = if let Some(itv) = span {
            self.read_time_interval(itv)?
        } else {
            Span::instant(self.chronicle.start)
        };
        self.enforce(&up::read_expression(cond)?, Some(span))?;
        Ok(())
    }

    fn set_duration(&mut self, interval: &Py) -> Result<(), Error> {
        let start = self.chronicle.start;
        let min = up::read_expression(&interval.getattr("lower")?).with_context(|| "Duration without a lower bound")?;
        let max =
            up::read_expression(&interval.getattr("upper")?).with_context(|| "Duration without an upper bound")?;

        let mut min: FAtom = self.reify(&min, Some(Span::instant(start)))?.try_into()?;
        let mut max: FAtom = self.reify(&max, Some(Span::instant(start)))?.try_into()?;

        if up::is(interval, "is_left_open")? {
            min = min + FAtom::EPSILON;
        }
        if up::is(interval, "is_right_open")? {
            max = max - FAtom::EPSILON;
        }

        let min = LinearSum::from(min);
        let max = LinearSum::from(max);

        self.chronicle
            .constraints
            .push(Constraint::duration(Duration::Bounded { lb: min, ub: max }));
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

    fn add_subtask(&mut self, subtask: &Py) -> Result<(), Error> {
        let task_index = self.chronicle.subtasks.len() as u32;
        let start = self.create_timepoint(VarType::TaskStart(task_index));
        let end = self.create_timepoint(VarType::TaskEnd(task_index));
        let parameters = up::list(&subtask.getattr("parameters")?)?;
        let mut task_name: Vec<Atom> = Vec::with_capacity(parameters.len() + 1);
        let head = str_to_symbol(&subtask_task_name(subtask)?, &self.context.symbols)?;
        task_name.push(Atom::Sym(head));
        for param in &parameters {
            let param = self.reify(&up::read_expression(param)?, None)?;
            task_name.push(param);
        }
        let id = subtask_id(subtask)?;
        self.chronicle.subtasks.push(SubTask {
            id: Some(id.clone()),
            start,
            end,
            task_name,
        });
        self.declare_time_interval(id, start, end)?;
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
                    "up:equals" | "up:iff" => {
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
                        let not_param0 = !Lit::try_from(self.reify(&params[0], span)?)?;
                        let value = Lit::try_from(value)?;
                        self.chronicle.constraints.push(Constraint {
                            variables: vec![not_param0.into()],
                            tpe: ConstraintType::Or,
                            value: Some(value),
                        });
                    }
                    "up:implies" => {
                        ensure!(params.len() == 2, "operator `implies` must have exactly 2 arguments");
                        let not_param0 = !Lit::try_from(self.reify(&params[0], span)?)?;
                        let params = vec![Atom::Bool(not_param0), self.reify(&params[1], span)?];
                        let value = Lit::try_from(value)?;
                        self.chronicle.constraints.push(Constraint {
                            variables: params,
                            tpe: ConstraintType::Or,
                            value: Some(value),
                        })
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
                    "up:present" => {
                        let param = params.first().context("Missing container parameter for up:present")?;
                        ensure!(kind(param)? == ExpressionKind::ContainerId);
                        let container = match param.atom.as_ref().unwrap().content.as_ref().unwrap() {
                            Content::Symbol(name) => name,
                            _ => bail!("Malformed protobuf"),
                        };
                        let lit =
                            self.env.presences.get(container).with_context(|| {
                                format!("No presence variable recorded for container {container:?}")
                            })?;
                        self.chronicle.constraints.push(Constraint::atom(Atom::Bool(*lit)));
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
                read_atom(atom, self.context.get_symbol_table()).with_context(|| format!("Unknown atom {atom:?}"))
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

                if operator == "up:present" {
                    let param = params.first().context("Missing container parameter for up:present")?;
                    ensure!(kind(param)? == ExpressionKind::ContainerId);
                    let container = match param.atom.as_ref().unwrap().content.as_ref().unwrap() {
                        Content::Symbol(name) => name,
                        _ => bail!("Malformed protobuf"),
                    };
                    let lit = self
                        .env
                        .presences
                        .get(container)
                        .with_context(|| format!("No presence variable recorded for container {container:?}"))?;
                    Ok(Atom::Bool(*lit))
                } else if operator == "up:start"
                    || operator == "up:end"
                    || operator == "up:global_start"
                    || operator == "up:global_end"
                {
                    // extract the timepoint
                    ensure!(
                        params.len() <= 1,
                        "Too many parameters for temporal qualifier: {operator}"
                    );
                    let timepoint = if let Some(param) = params.first() {
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
                        "up:implies" => {
                            ensure!(params.len() == 2, "`implies` operator should have exactly 2 arguments");
                            let value = self.create_bool_variable(VarType::Reification);
                            let constraint = Constraint {
                                variables: vec![Atom::Bool(!params[0].try_into()?), params[1]],
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
                        "up:equals" | "up:iff" => {
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
                            let value: Var = self.create_variable(Type::UNBOUNDED_INT, VarType::Reification).into();
                            let mut sum = -LinearSum::from(value);
                            for param in params {
                                sum += LinearSum::try_from(param)?;
                            }
                            self.chronicle.constraints.push(Constraint::linear_eq_zero(sum));
                            Ok(value.into())
                        }
                        "up:minus" => {
                            ensure!(params.len() == 2, "`-` operator should have exactly 2 arguments");
                            let value: Var = self.create_variable(Type::UNBOUNDED_INT, VarType::Reification).into();
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
            kind => bail!("Unsupported expression kind: {kind:?}"),
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

    fn read_timing(&self, timing: &Py) -> Result<FAtom, Error> {
        let (delay_num, delay_denom) = {
            let delay: Ratio<i64> = timing.getattr("delay")?.extract()?;
            let (num, denom) = (*delay.numer(), *delay.denom());
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
        let kind: String = timing
            .getattr("timepoint")?
            .getattr("kind")?
            .getattr("name")?
            .extract()?;
        let tp = match kind.as_str() {
            "GLOBAL_START" => self.context.origin(),
            "GLOBAL_END" => self.context.horizon(),
            "START" => self.chronicle.start,
            "END" => self.chronicle.end,
            _ => bail!("Unsupported timepoint kind"),
        };
        assert_eq!(tp.denom, delay_denom);
        Ok(FAtom::new(tp.num + delay_num, tp.denom))
    }

    /// Returns the corresponding start and end timepoints representing the interval.
    /// Note that if the interval left/right opened, the corresponding timepoint is shifted by the smallest representable value.
    fn read_time_interval(&self, interval: &Py) -> Result<Span, Error> {
        let start = self.read_timing(&interval.getattr("lower")?)?;
        let start = if up::is(interval, "is_left_open")? {
            start + FAtom::EPSILON
        } else {
            start
        };
        let end = self.read_timing(&interval.getattr("upper")?)?;
        let end = if up::is(interval, "is_right_open")? {
            end - FAtom::EPSILON
        } else {
            end
        };
        Ok(Span::interval(start, end))
    }

    fn read_fluent_symbol(&self, expr: &Expression) -> Result<Arc<Fluent>, Error> {
        ensure!(kind(expr)? == ExpressionKind::FluentSymbol);

        match read_atom(expr.atom.as_ref().unwrap(), self.context.get_symbol_table())? {
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
fn get_fixed_duration(duration: &Py) -> Result<Option<IntCst>, Error> {
    let min = up::read_expression(&duration.getattr("lower")?)?;
    let max = up::read_expression(&duration.getattr("upper")?)?;
    if min == max && !up::is(duration, "is_left_open")? && !up::is(duration, "is_right_open")? {
        Ok(as_int(&min).ok())
    } else {
        Ok(None)
    }
}

#[derive(Copy, Clone, Debug)]
enum EffectKind {
    Assign,
    Increase,
    Decrease,
}

fn effect_kind(eff: &Py) -> Result<EffectKind, Error> {
    if up::is(eff, "is_assignment")? {
        Ok(EffectKind::Assign)
    } else if up::is(eff, "is_increase")? {
        Ok(EffectKind::Increase)
    } else if up::is(eff, "is_decrease")? {
        Ok(EffectKind::Decrease)
    } else {
        bail!("Unknown effect kind: {eff}")
    }
}

fn subtask_task_name(subtask: &Py) -> Result<String, Error> {
    up::name(&subtask.getattr("task")?)
}

fn subtask_id(subtask: &Py) -> Result<String, Error> {
    Ok(subtask.getattr("identifier")?.extract()?)
}

fn read_action(
    container: Container,
    action: &Py,
    costs: &ActionCosts,
    context: &mut Ctx,
) -> Result<ChronicleTemplate, Error> {
    let action_name = up::name(action)?;
    let duration = if action.hasattr("duration")? {
        Some(action.getattr("duration")?)
    } else {
        None
    };
    let action_kind = {
        if duration.is_some() {
            ChronicleKind::DurativeAction
        } else {
            ChronicleKind::Action
        }
    };
    let mut variables: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(container / VarType::Presence);
    variables.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context.new_optional_fvar(
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
            let duration = duration
                .as_ref()
                .with_context(|| format!("Missing duration in durative action {}", action_name))?;
            if let Some(dur) = get_fixed_duration(duration)? {
                // a duration constraint is added later in the function for more complex durations
                start + dur
            } else {
                let end = context.new_optional_fvar(
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

    let parameters = up::list(&action.getattr("parameters")?)?;
    let mut name: Vec<Atom> = Vec::with_capacity(1 + parameters.len());
    let base_name = &Sym::from(action_name.clone());
    name.push(
        context
            .typed_sym(
                context
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
    for param in &parameters {
        let param_name = up::name(param)?;
        factory
            .add_parameter(param_name.clone(), up::proto_type(&param.getattr("type")?)?)
            .with_context(|| format!("Adding parameter: {}", param_name))?;
    }

    // set the action's achieved task to be the same as the its name
    // note that we wait until all parameters have been add to the name before doing this
    factory.chronicle.task = Some(factory.chronicle.name.clone());

    if duration.is_some() {
        for (timing, eff) in up::dict_of_lists(&action.getattr("effects")?)? {
            factory
                .add_up_effect(&eff, Some(&timing))
                .with_context(|| format!("Adding effect: {eff:?}"))?;
        }

        for (span, condition) in up::dict_of_lists(&action.getattr("conditions")?)? {
            factory
                .add_condition(&condition, Some(&span))
                .with_context(|| format!("Adding condition: {condition:?}"))?;
        }
    } else {
        for eff in up::list(&action.getattr("effects")?)? {
            factory
                .add_up_effect(&eff, None)
                .with_context(|| format!("Adding effect: {eff:?}"))?;
        }

        for condition in up::list(&action.getattr("preconditions")?)? {
            factory
                .add_condition(&condition, None)
                .with_context(|| format!("Adding condition: {condition:?}"))?;
        }
    }

    if let Some(duration) = duration.as_ref() {
        factory
            .set_duration(duration)
            .with_context(|| format!("Setting duration: {duration:?}"))?;
    }

    let cost_expr = costs.costs.get(&action_name).or(costs.default.as_ref());
    if let Some(cost) = cost_expr {
        factory.set_cost(cost)?;
    }

    factory.build_template(action_name)
}

fn read_activity(
    container: Container,
    activity: &Py,
    context: &mut Ctx,
    global_env: &Env,
) -> Result<ChronicleInstance, Error> {
    let activity_name = up::name(activity)?;
    // similar to an action but all variables have been previously declared in the global_env
    let (start, end) = global_env.intervals[&activity_name];
    let prez = global_env.presences[&activity_name];

    let parameters = up::list(&activity.getattr("parameters")?)?;
    let mut name: Vec<Atom> = Vec::with_capacity(1 + parameters.len());
    let base_name = &Sym::from(activity_name.clone());
    name.push(
        context
            .typed_sym(
                context
                    .get_symbol_table()
                    .id(base_name)
                    .ok_or_else(|| base_name.invalid("Unknown action"))?,
            )
            .into(),
    );
    for param in &parameters {
        name.push(global_env.parameters[&up::name(param)?].into());
    }

    let ch = Chronicle {
        kind: ChronicleKind::DurativeAction,
        presence: prez,
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

    for (timing, eff) in up::dict_of_lists(&activity.getattr("effects")?)? {
        factory
            .add_up_effect(&eff, Some(&timing))
            .with_context(|| format!("Adding effect: {eff:?}"))?;
    }
    //
    for (span, condition) in up::dict_of_lists(&activity.getattr("conditions")?)? {
        factory
            .add_condition(&condition, Some(&span))
            .with_context(|| format!("Adding condition: {condition:?}"))?;
    }
    // the scope is ignored as it always corresponds to the activity's presence
    for scoped in up::list(&activity.getattr("scoped_constraints")?)? {
        let (constraint, _scope) = scoped.extract::<(Py, Py)>()?;
        factory
            .enforce(&up::read_expression(&constraint)?, Some(Span::interval(start, end)))
            .with_context(|| format!("Adding constraint: {constraint:?}"))?;
    }

    let duration = activity.getattr("duration")?;
    factory
        .set_duration(&duration)
        .with_context(|| format!("Setting duration: {duration:?}"))?;

    factory.build_instance(ChronicleOrigin::Original)
}

fn read_method(container: Container, method: &Py, context: &mut Ctx) -> Result<ChronicleTemplate, Error> {
    let method_name = up::name(method)?;
    let subtasks = up::list(&method.getattr("subtasks")?)?;
    let mut variables: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(container / VarType::Presence);
    variables.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context.new_optional_fvar(
        0,
        INT_CST_MAX,
        TIME_SCALE.get(),
        prez,
        container / VarType::ChronicleStart,
    );
    variables.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = if subtasks.is_empty() {
        start // no subtasks, the method is instantaneous
    } else {
        let end = context.new_optional_fvar(
            0,
            INT_CST_MAX,
            TIME_SCALE.get(),
            prez,
            container / VarType::ChronicleEnd,
        );
        variables.push(end.into());
        end.into()
    };

    let parameters = up::list(&method.getattr("parameters")?)?;
    let mut name: Vec<Atom> = Vec::with_capacity(1 + parameters.len());
    let base_name = &Sym::from(method_name.clone());
    name.push(
        context
            .typed_sym(
                context
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
    for param in &parameters {
        factory.add_parameter(up::name(param)?, up::proto_type(&param.getattr("type")?)?)?;
    }

    let achieved_task = method
        .getattr("achieved_task")
        .with_context(|| format!("Missing achieved task in method: {}", &method_name))?;
    let achieved_parameters = up::list(&achieved_task.getattr("parameters")?)?;
    let mut task_name: Vec<Atom> = Vec::with_capacity(achieved_parameters.len() + 1);
    let head = str_to_symbol(&up::name(&achieved_task.getattr("task")?)?, &factory.context.symbols)?;
    task_name.push(head.into());
    for param in &achieved_parameters {
        let param = factory.reify(&up::parameter_expression(param)?, None)?;
        task_name.push(param);
    }
    factory.chronicle.task = Some(task_name);

    for subtask in &subtasks {
        factory.add_subtask(subtask).with_context(|| {
            format!(
                "Adding subtask: {} ({})",
                subtask_task_name(subtask).unwrap_or_default(),
                subtask_id(subtask).unwrap_or_default()
            )
        })?;
    }

    for condition in up::list(&method.getattr("preconditions")?)? {
        factory
            .add_condition(&condition, None)
            .with_context(|| format!("Adding condition {condition:?}"))?;
    }

    for constraint in up::list(&method.getattr("constraints")?)? {
        factory
            .enforce(&up::read_expression(&constraint)?, None)
            .with_context(|| format!("Adding constraint {constraint:?}"))?;
    }

    factory.build_template(method_name)
}

fn kind(e: &Expression) -> Result<ExpressionKind, Error> {
    Ok(e.kind)
}

fn as_int(e: &Expression) -> Result<IntCst, Error> {
    if kind(e)? == ExpressionKind::Constant && e.r#type.starts_with("up:integer") {
        match e.atom.as_ref().unwrap().content.as_ref().unwrap() {
            Content::Int(i) => Ok(*i as IntCst),
            _ => bail!("Malformed message"),
        }
    } else {
        bail!("Expression is not a constant int")
    }
}
