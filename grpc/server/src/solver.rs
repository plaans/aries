use crate::solver::Strat::{Activity, Forward};
use aries_grpc_api::{Action, Answer, Expression, Problem};

use anyhow::Result;
use anyhow::{anyhow, Context, Error};
use core::fmt::Formatter;
use std::collections::HashSet;
use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use structopt::StructOpt;

use aries_model::extensions::SavedAssignment;
use aries_model::extensions::Shaped;
use aries_planners::encode::{encode, populate_with_task_network, populate_with_template_instances};
use aries_planners::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use aries_planners::forward_search::ForwardSearcher;
use aries_planners::Solver;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;
use aries_solver::parallel_solver::ParSolver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};

use aries_model::bounds::Lit;
use aries_model::lang::*;
use aries_model::symbols::{SymbolTable, TypedSym};
use aries_model::types::TypeHierarchy;
use aries_utils::input::Sym;

/// Names for built in types. They contain UTF-8 symbols for sexiness (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static FLUENT_TYPE: &str = "★fluent★";
static OBJECT_TYPE: &str = "★object★";

// TODO: Replace panic with error

/// An automated planner for PDDL and HDDL problems.
#[derive(Debug, StructOpt)]
#[structopt(name = "grpc", rename_all = "kebab-case")]
struct Opt {
    /// If set, a machine readable plan will be written to the file.
    #[structopt(long = "output", short = "o")]
    plan_out_file: Option<PathBuf>,
    /// Minimum depth of the instantiation. (depth of HTN tree or number of standalone actions with the same name).
    #[structopt(long)]
    min_depth: Option<u32>,
    /// Maximum depth of instantiation
    #[structopt(long)]
    max_depth: Option<u32>,
    /// If set, the solver will attempt to minimize the makespan of the plan.
    #[structopt(long = "optimize")]
    optimize_makespan: bool,
    /// If true, then the problem will be constructed, a full propagation will be made and the resulting
    /// partial plan will be displayed.
    #[structopt(long = "no-search")]
    no_search: bool,
    /// If provided, the solver will only run the specified strategy instead of default set of strategies.
    /// When repeated, several strategies will be run in parallel.
    #[structopt(long = "strategy", short = "s")]
    strategies: Vec<Strat>,
}

// Implement new for Opt
impl Opt {
    fn new() -> Self {
        Opt {
            plan_out_file: None,
            min_depth: None,
            max_depth: None,
            optimize_makespan: false,
            no_search: false,
            strategies: Vec::new(),
        }
    }
}
pub fn solve(problem: aries_grpc_api::Problem) -> Result<aries_grpc_api::Answer, Error> {
    let answer = Answer::default();
    let opt = Opt::new();

    //TODO: Check if htn problem
    let htn_mode: bool = false;

    //Convert to chronicles
    let mut spec = problem_to_chronicles(problem)?;

    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut spec);
    println!("==========================");

    // if not explicitly given, compute the min/max search depth
    let max_depth = opt.max_depth.unwrap_or(u32::MAX);
    let min_depth = if let Some(min_depth) = opt.min_depth {
        min_depth
    } else if htn_mode && hierarchical_is_non_recursive(&spec) {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        0
    };

    for n in min_depth..=max_depth {
        let depth_string = if n == u32::MAX {
            "∞".to_string()
        } else {
            n.to_string()
        };
        println!("{} Solving with {} actions", depth_string, depth_string);
        let start = Instant::now();
        let mut pb = FiniteProblem {
            model: spec.context.model.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            chronicles: spec.chronicles.clone(),
            tables: spec.context.tables.clone(),
        };
        if htn_mode {
            populate_with_task_network(&mut pb, &spec, n)?;
        } else {
            populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        if opt.no_search {
            propagate_and_print(&pb);
            break;
        } else {
            let result = _solve(&pb, &opt, htn_mode);
            println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
            if let Some(x) = result {
                // println!("{}", format_partial_plan(&pb, &x)?);
                println!("  Solution found");
                let plan = if htn_mode {
                    format!(
                        "\n**** Decomposition ****\n\n\
                        {}\n\n\
                        **** Plan ****\n\n\
                        {}",
                        format_hddl_plan(&pb, &x)?,
                        format_pddl_plan(&pb, &x)?
                    )
                } else {
                    format_pddl_plan(&pb, &x)?
                };
                println!("{}", plan);
                if let Some(plan_out_file) = opt.plan_out_file {
                    let mut file = File::create(plan_out_file)?;
                    file.write_all(plan.as_bytes())?;
                }
                break;
            }
        }
    }
    Ok(answer)
}

//Convert Problem_ to chronicles
fn problem_to_chronicles(problem: Problem) -> Result<aries_planning::chronicles::Problem> {
    // top types in pddl
    let mut types: Vec<(Sym, Option<Sym>)> = vec![
        (TASK_TYPE.into(), None),
        (ABSTRACT_TASK_TYPE.into(), Some(TASK_TYPE.into())),
        (ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (DURATIVE_ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (METHOD_TYPE.into(), None),
        (FLUENT_TYPE.into(), None),
        (OBJECT_TYPE.into(), None),
    ];
    // let top_type = OBJECT_TYPE.into();

    // determine the top types in the user-defined hierarchy.
    // this is typically "object" by convention but might something else (e.g. "obj" in some hddl problems).
    let mut symbols: Vec<TypedSymbol> = vec![];
    {
        // TODO: Check if they are of top types in user hierarchy
        //Check if types are already in types
        for obj in &problem.objects {
            let type_ = Sym::from(obj.name.clone());
            let type_symbol = Sym::from(obj.name.clone());

            //check if type is already in types
            if !types.iter().any(|(t, _)| t == &type_) {
                types.push((type_, Some(OBJECT_TYPE.into())));
            }

            //add type to symbols
            symbols.push(TypedSymbol {
                symbol: type_symbol,
                tpe: Some(OBJECT_TYPE.into()),
            });
        }
    }

    let ts = TypeHierarchy::new(types)?;
    // TODO: currently, the protobuf does not allow defining a type hierarchy as in PDDL
    //       We should fix this in the protobuf and then import each type's parent un the hierarchy

    {
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
    let symbol_table = SymbolTable::new(ts.clone(), symbols)?;

    let from_upf_type = |name: &str| {
        if name == "bool" {
            Ok(Type::Bool)
        } else if name == "int" {
            Ok(Type::Int)
        } else if let Some(tpe) = ts.id_of(name) {
            Ok(Type::Sym(tpe))
        } else {
            Err(anyhow!("Unsupported type {}", name))
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
                    .with_context(|| format!("Invalid parameter type for fluent {}", fluent.name))?,
            );
        }

        args.push(
            from_upf_type(&fluent.value_type)
                .with_context(|| format!("Invalid return type for fluent {}", fluent.name))?,
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
    for init_state in problem.initial_state {
        let expr = init_state.x.context("Initial state assignment has no valid fluent")?;
        let value = init_state.v.context("Initial state assignment has no valid value")?;

        let expr = read_abstract(expr, &symbol_table)?;
        let value = read_abstract(value, &symbol_table)?;

        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var: expr.sv,
            value: value.output_value.unwrap(),
        })
    }

    // goals translate as condition at the global end time
    for goal in problem.goals {
        let goal = read_abstract(goal, &symbol_table)?;

        init_ch.conditions.push(Condition {
            start: init_ch.end,
            end: init_ch.end,
            state_var: goal.sv,
            value: goal.output_value.context("Missing goal expected value")?,
        })
    }

    // TODO: Task networks?

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

//Convert chronicles to Answer_

pub struct Abstract {
    sv: Vec<SAtom>,
    symbol: SAtom,
    operator: Option<Atom>,
    output_value: Option<Atom>,
}

impl Display for Abstract {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "\nAbstract:\n")?;
        write!(f, "SV: {:?} ", self.sv)?;
        write!(f, "Operator: {:?}", self.operator)
    }
}

//TODO: Rewrite lame code
fn read_abstract(expr: Expression, symbol_table: &SymbolTable) -> Result<Abstract, Error> {
    //Parse expression in the format of abstract syntax tree
    let mut sv = Vec::new();
    let mut operator: Option<Atom> = None;
    let mut value: Option<Atom> = None;

    let payload_type = expr.clone().payload.unwrap().r#type;
    let payload = expr.payload.unwrap().value;
    let symbol = Sym::from(payload.clone());
    let symbol_atom = SAtom::from(TypedSym {
        sym: symbol_table.id(&symbol).unwrap(),
        // BUG: thread 'tokio-runtime-worker' panicked at 'called `Option::unwrap()` on a `None` value'
        tpe: symbol_table.types.id_of(&symbol).unwrap(),
    });
    sv.push(symbol_atom);

    //Check if symbol in symbol table
    for arg in expr.args {
        let abstract_ = read_abstract(arg, symbol_table)?;
        sv.push(abstract_.symbol)
    }

    if !symbol_table.symbols.contains(&symbol) {
        if payload_type == "bool" {
            // tpe = Some(Type::Bool);
            value = if symbol == (Sym::from("true")) {
                Some(Atom::Bool(true.into()))
            } else {
                Some(Atom::Bool(false.into()))
            };
        } else if payload_type == "int" {
            // tpe = Some(Type::Int);
            value = Some(Atom::Int(payload.parse::<i32>().unwrap().into()));
        } else {
            operator = Some(Atom::Sym(symbol_atom));
        }
    }

    Ok(Abstract {
        sv,
        symbol: symbol_atom,
        operator,
        output_value: value,
    })
}

// TODO: Replace Action_ with Enum of Action, Method, and DurativeAction
pub enum ChronicleAs<'a> {
    Action(&'a Action),
    // Method(&'a Method_),
    // DurativeAction(&'a DurativeAction_),
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

fn read_chronicle_template(c: Container, action: ChronicleAs, context: &mut Ctx) -> Result<ChronicleTemplate> {
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
        ChronicleKind::Problem => panic!("unsupported case"),
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
                    .ok_or_else(|| base_name.invalid("Unknown atom"))?,
            )
            .into(),
    );

    // Process, the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for arg in action.parameters.clone() {
        let arg = Sym::from(arg.clone());
        let tpe = context
            .model
            .get_symbol_table()
            .types
            .id_of(&arg)
            .ok_or_else(|| arg.invalid("Unknown atom"))?;
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
    for _eff in action.effects.clone() {
        let eff = _eff.x.unwrap();
        let eff = read_abstract(eff, context.model.get_symbol_table())?;
        let eff_value = _eff.v.unwrap();
        let eff_value = read_abstract(eff_value, context.model.get_symbol_table())?;
        ch.effects.push(Effect {
            transition_start: start,
            persistence_start: end,
            state_var: eff.sv,
            value: eff_value.output_value.unwrap(),
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

    for condition in action.preconditions.clone() {
        let condition = read_abstract(condition, context.model.get_symbol_table())?;
        ch.conditions.push(Condition {
            start,
            end,
            state_var: condition.sv,
            value: condition.output_value.unwrap(),
        })
    }

    Ok(ChronicleTemplate {
        label: Some(action.name.clone()),
        parameters: params,
        chronicle: ch,
    })
}

fn init_solver(pb: &FiniteProblem) -> Box<Solver> {
    let model = encode(pb).unwrap(); // TODO: report error
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };

    let mut solver = Box::new(aries_solver::solver::Solver::new(model));
    solver.add_theory(|tok| StnTheory::new(tok, stn_config));
    solver
}

/// Default set of strategies for HTN problems
const HTN_DEFAULT_STRATEGIES: [Strat; 2] = [Strat::Activity, Strat::Forward];
/// Default set of strategies for generative (flat) problems.
const GEN_DEFAULT_STRATEGIES: [Strat; 1] = [Strat::Activity];

#[derive(Copy, Clone, Debug)]
enum Strat {
    /// Activity based search
    Activity,
    /// Mimics forward search in HTN problems.
    Forward,
}

impl Strat {
    /// Configure the given solver to follow the strategy.
    pub fn adapt_solver(self, solver: &mut Solver, problem: &FiniteProblem) {
        match self {
            Activity => {
                // nothing, activity based search is the default configuration
            }
            Forward => solver.set_brancher(ForwardSearcher::new(Arc::new(problem.clone()))),
        }
    }
}

impl FromStr for Strat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "act" | "activity" => Ok(Activity),
            "2" | "fwd" | "forward" => Ok(Forward),
            _ => Err(format!("Unknown search strategy: {}", s)),
        }
    }
}

fn _solve(pb: &FiniteProblem, opt: &Opt, htn_mode: bool) -> Option<std::sync::Arc<SavedAssignment>> {
    let solver = init_solver(pb);
    let strats: &[Strat] = if !opt.strategies.is_empty() {
        &opt.strategies
    } else if htn_mode {
        &HTN_DEFAULT_STRATEGIES
    } else {
        &GEN_DEFAULT_STRATEGIES
    };
    let mut solver = if htn_mode {
        aries_solver::parallel_solver::ParSolver::new(solver, strats.len(), |id, s| strats[id].adapt_solver(s, pb))
    } else {
        ParSolver::new(solver, 1, |_, _| {})
    };

    let found_plan = if opt.optimize_makespan {
        let res = solver.minimize(pb.horizon.num).unwrap();
        res.map(|tup| tup.1)
    } else {
        solver.solve().unwrap()
    };

    if let Some(solution) = found_plan {
        solver.print_stats();
        Some(solution)
    } else {
        None
    }
}

fn propagate_and_print(pb: &FiniteProblem) {
    let mut solver = init_solver(pb);
    if solver.propagate_and_backtrack_to_consistent() {
        let str = format_partial_plan(pb, &solver.model).unwrap();
        println!("{}", str);
    } else {
        panic!("Invalid problem");
    }
}
