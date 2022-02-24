use crate::Strat::{Activity, Forward};
use aries_core::*;
use aries_model::extensions::{SavedAssignment, Shaped};
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_planners::encode::{encode, populate_with_task_network, populate_with_template_instances};
use aries_planners::fmt::{format_hddl_plan, format_pddl_plan};
use aries_planners::forward_search::ForwardSearcher;
use aries_planners::Solver;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use aries_planning::chronicles::constraints::Constraint;
use aries_planning::chronicles::*;
use aries_solver::parallel_solver::ParSolver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};
use aries_utils::input::Sym;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

//#region Types definition
// PDDL types
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static PREDICATE_TYPE: &str = "★predicate★";
static OBJECT_TYPE: &str = "★object★";
static FUNCTION_TYPE: &str = "★function★";
// My types
static LOCATION_TYPE: &str = "location";
static LOCATABLE_TYPE: &str = "locatable";
static BOT_TYPE: &str = "bot";
static PACKAGE_TYPE: &str = "package";
// My objects
static ROBOT_OBJ: &str = "robot";
static PACKAGE_OBJ: &str = "package";
static LOC1_OBJ: &str = "loc1";
static LOC2_OBJ: &str = "loc2";
// My constants: None
// My predicates
static ON_PRED: &str = "on";
static HOLDING_PRED: &str = "holding";
static EMPTY: &str = "empty";
// My actions
static PICK_UP_ACTION: &str = "pick-up";
static DROP_ACTION: &str = "drop";
static MOVE_ACTION: &str = "move";
// My durative actions
static MOVE_DUR_ACTION: &str = "move";
// My tasks
static TRANSFER_TASK: &str = "transfer";
static GOTO_TASK: &str = "goto";
// My methods
static M_TRANSFER_METHOD: &str = "m-transfer";
static M_ALREADY_TRANSFERED_METHOD: &str = "m-already-transfered";
static M_GOTO_METHOD: &str = "m-goto";
static M_ALREADY_THERE_METHOD: &str = "m-already-there";
// My functions: None
//#endregion

//#region Tests
#[test]
fn test_no_htn_problem() {
    let mut pb = get_no_htn_problem();
    run_problem(&mut pb, false);
}

#[test]
fn test_htn_problem() {
    let mut pb = get_htn_problem();
    run_problem(&mut pb, true);
}
//#endregion

//#region Main & Solver
fn run_problem(problem: &mut Problem, htn_mode: bool) {
    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(problem);
    println!("==========================");

    let max_depth = u32::MAX;
    let min_depth = if htn_mode && hierarchical_is_non_recursive(&problem) {
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
            model: problem.context.model.clone(),
            origin: problem.context.origin(),
            horizon: problem.context.horizon(),
            chronicles: problem.chronicles.clone(),
            tables: problem.context.tables.clone(),
        };
        if htn_mode {
            populate_with_task_network(&mut pb, &problem, n).unwrap();
        } else {
            populate_with_template_instances(&mut pb, &problem, |_| Some(n)).unwrap();
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        let result = solve(&pb, htn_mode);
        println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
        if let Some(x) = result {
            // println!("{}", format_partial_plan(&pb, &x)?);
            println!("  Solution found");
            if htn_mode {
                println!(
                    "{}",
                    format!(
                        "\n**** Decomposition ****\n\n\
                    {}\n\n\
                    **** Plan ****\n\n\
                    {}",
                        format_hddl_plan(&pb, &x).unwrap(),
                        format_pddl_plan(&pb, &x).unwrap()
                    )
                );
            } else {
                println!("{}", format_pddl_plan(&pb, &x).unwrap());
            }
            break;
        }
    }
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

fn solve(pb: &FiniteProblem, htn_mode: bool) -> Option<std::sync::Arc<SavedAssignment>> {
    let solver = init_solver(pb);
    let strats: &[Strat] = if htn_mode {
        &HTN_DEFAULT_STRATEGIES
    } else {
        &GEN_DEFAULT_STRATEGIES
    };
    let mut solver = if htn_mode {
        aries_solver::parallel_solver::ParSolver::new(solver, strats.len(), |id, s| strats[id].adapt_solver(s, pb))
    } else {
        ParSolver::new(solver, 1, |_, _| {})
    };

    let found_plan = solver.solve().unwrap();

    if let Some(solution) = found_plan {
        solver.print_stats();
        Some(solution)
    } else {
        None
    }
}
//#endregion

//#region Problems
fn get_no_htn_problem() -> Problem {
    // Creation of the types
    let ts = create_type_hierarchy();

    // Creation of the symbols
    let mut symbols = create_common_symbols();
    symbols.push((MOVE_DUR_ACTION.into(), DURATIVE_ACTION_TYPE.into()));
    let symbol_table = SymbolTable::new(ts, symbols).unwrap();

    // Creation of the state variables
    let state_variables = create_state_variables(&symbol_table);
    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

    // Creation of the problem
    let mut pb = Chronicle {
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

    // Creation of the goal
    pb.conditions.push(Condition {
        start: pb.end,
        end: pb.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(PACKAGE_OBJ).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(LOC2_OBJ).unwrap())
                .into(),
        ],
        value: true.into(),
    });

    // Instantiation of the problem
    create_initial_state(&mut context, &mut pb);
    let pb = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: pb,
    };

    // Creation of the chronicle templates
    let templates: Vec<ChronicleTemplate> = vec![
        get_move_duractive_action_template(0, &mut context),
        get_pick_up_action_template(1, &mut context),
        get_drop_action_template(2, &mut context),
    ];

    // Return the problem
    Problem {
        context,
        templates,
        chronicles: vec![pb],
    }
}

fn get_htn_problem() -> Problem {
    // Creation of the types
    let ts = create_type_hierarchy();

    // Creation of the symbols
    let mut symbols = create_common_symbols();
    symbols.extend(vec![
        (MOVE_ACTION.into(), ACTION_TYPE.into()),
        (TRANSFER_TASK.into(), ABSTRACT_TASK_TYPE.into()),
        (GOTO_TASK.into(), ABSTRACT_TASK_TYPE.into()),
        (M_TRANSFER_METHOD.into(), METHOD_TYPE.into()),
        (M_ALREADY_TRANSFERED_METHOD.into(), METHOD_TYPE.into()),
        (M_GOTO_METHOD.into(), METHOD_TYPE.into()),
        (M_ALREADY_THERE_METHOD.into(), METHOD_TYPE.into()),
    ]);
    let symbol_table = SymbolTable::new(ts, symbols).unwrap();

    // Creation of the state variables
    let state_variables = create_state_variables(&symbol_table);
    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

    // Creation of the problem
    let init_container = Container::Instance(0);
    let mut pb = Chronicle {
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

    // Creation of the goal
    let task_name = vec![
        context
            .typed_sym(context.model.get_symbol_table().id(TRANSFER_TASK).unwrap())
            .into(),
        context
            .typed_sym(context.model.get_symbol_table().id(PACKAGE_OBJ).unwrap())
            .into(),
        context
            .typed_sym(context.model.get_symbol_table().id(LOC2_OBJ).unwrap())
            .into(),
    ];
    pb.subtasks.push(create_subtask(
        &mut context,
        init_container,
        pb.presence,
        None,
        task_name,
    ));

    // Instantiation of the problem
    create_initial_state(&mut context, &mut pb);
    let pb = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: pb,
    };

    // Creation of the chronicle templates
    let templates: Vec<ChronicleTemplate> = vec![
        get_move_action_template(0, &mut context),
        get_m_goto_method_template(1, &mut context),
        get_m_already_there_method_template(2, &mut context),
        get_pick_up_action_template(3, &mut context),
        get_drop_action_template(4, &mut context),
        get_m_transfer_method_template(5, &mut context),
        get_m_already_transfered_method_template(6, &mut context),
    ];

    // Return the problem
    Problem {
        context,
        templates,
        chronicles: vec![pb],
    }
}
//#endregion

//#region Common functions
fn create_type_hierarchy() -> TypeHierarchy {
    let types: Vec<(Sym, Option<Sym>)> = vec![
        // PDDL
        (TASK_TYPE.into(), None),
        (ABSTRACT_TASK_TYPE.into(), Some(TASK_TYPE.into())),
        (ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (DURATIVE_ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (METHOD_TYPE.into(), None),
        (PREDICATE_TYPE.into(), None),
        (FUNCTION_TYPE.into(), None),
        (OBJECT_TYPE.into(), None),
        // My own
        (LOCATION_TYPE.into(), Some(OBJECT_TYPE.into())),
        (LOCATABLE_TYPE.into(), Some(OBJECT_TYPE.into())),
        (BOT_TYPE.into(), Some(LOCATABLE_TYPE.into())),
        (PACKAGE_TYPE.into(), Some(LOCATABLE_TYPE.into())),
    ];
    TypeHierarchy::new(types).unwrap()
}

fn create_common_symbols() -> Vec<(Sym, Sym)> {
    vec![
        // Objects
        (ROBOT_OBJ.into(), BOT_TYPE.into()),
        (PACKAGE_OBJ.into(), PACKAGE_TYPE.into()),
        (LOC1_OBJ.into(), LOCATION_TYPE.into()),
        (LOC2_OBJ.into(), LOCATION_TYPE.into()),
        // Predicates
        (ON_PRED.into(), PREDICATE_TYPE.into()),
        (HOLDING_PRED.into(), PREDICATE_TYPE.into()),
        (EMPTY.into(), PREDICATE_TYPE.into()),
        // Actions
        (PICK_UP_ACTION.into(), ACTION_TYPE.into()),
        (DROP_ACTION.into(), ACTION_TYPE.into()),
    ]
}

fn create_state_variables(symbol_table: &SymbolTable) -> Vec<StateFun> {
    vec![
        StateFun {
            sym: symbol_table.id(ON_PRED).unwrap(),
            tpe: vec![
                Type::Sym(symbol_table.types.id_of(LOCATABLE_TYPE).unwrap()),
                Type::Sym(symbol_table.types.id_of(LOCATION_TYPE).unwrap()),
                Type::Bool,
            ],
        },
        StateFun {
            sym: symbol_table.id(HOLDING_PRED).unwrap(),
            tpe: vec![
                Type::Sym(symbol_table.types.id_of(BOT_TYPE).unwrap()),
                Type::Sym(symbol_table.types.id_of(PACKAGE_TYPE).unwrap()),
                Type::Bool,
            ],
        },
        StateFun {
            sym: symbol_table.id(EMPTY).unwrap(),
            tpe: vec![Type::Sym(symbol_table.types.id_of(BOT_TYPE).unwrap()), Type::Bool],
        },
    ]
}

fn create_initial_state(context: &mut Ctx, pb: &mut Chronicle) {
    // Creation of the initial state
    pb.effects.push(Effect {
        transition_start: pb.start,
        persistence_start: pb.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(ROBOT_OBJ).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(LOC1_OBJ).unwrap())
                .into(),
        ],
        value: true.into(),
    });
    pb.effects.push(Effect {
        transition_start: pb.start,
        persistence_start: pb.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(PACKAGE_OBJ).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(LOC1_OBJ).unwrap())
                .into(),
        ],
        value: true.into(),
    });
    pb.effects.push(Effect {
        transition_start: pb.start,
        persistence_start: pb.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap())
                .into(),
            context
                .typed_sym(context.model.get_symbol_table().id(ROBOT_OBJ).unwrap())
                .into(),
        ],
        value: true.into(),
    });
}

fn create_subtask(
    context: &mut Ctx,
    c: Container,
    prez: Lit,
    mut params: Option<&mut Vec<Variable>>,
    task_name: Vec<SAtom>,
) -> SubTask {
    let id = None;
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::TaskStart);
    let end = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::TaskEnd);
    if let Some(ref mut p) = params {
        p.push(start.into());
        p.push(end.into());
    }
    let start = FAtom::from(start);
    let end = FAtom::from(end);
    SubTask {
        id,
        start,
        end,
        task_name,
    }
}
//#endregion

//#region Chronicle templates
fn get_move_duractive_action_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    // End
    let end = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
    params.push(end.into());
    let end = end.into();

    // Name & Arguments
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(MOVE_DUR_ACTION).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let from_arg = name[2];
    let to_arg = name[3];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Action,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(name.clone()),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Duration constraint
    ch.constraints.push(Constraint::duration(1));

    // Conditions
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            from_arg,
        ],
        value: true.into(),
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.start + FAtom::EPSILON,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            from_arg,
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.end,
        persistence_start: ch.end + FAtom::EPSILON,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            to_arg,
        ],
        value: true.into(),
    });

    // Template
    ChronicleTemplate {
        label: Some("move".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_move_action_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start & End
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end = start + FAtom::EPSILON;

    // Name & Arguments
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(MOVE_ACTION).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let from_arg = name[2];
    let to_arg = name[3];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Action,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(name.clone()),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Conditions
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            from_arg,
        ],
        value: true.into(),
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            from_arg,
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            to_arg,
        ],
        value: true.into(),
    });

    // Template
    ChronicleTemplate {
        label: Some("move".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_pick_up_action_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start & End
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end = start + FAtom::EPSILON;

    // Name & Argument
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(PICK_UP_ACTION).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(PACKAGE_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let package_arg = name[2];
    let loc_arg = name[3];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Action,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(name.clone()),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Conditions
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            loc_arg,
        ],
        value: true.into(),
    });
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            package_arg,
            loc_arg,
        ],
        value: true.into(),
    });
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap())
                .into(),
            bot_arg,
        ],
        value: true.into(),
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            package_arg,
            loc_arg,
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(HOLDING_PRED).unwrap())
                .into(),
            bot_arg,
            package_arg,
        ],
        value: true.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap())
                .into(),
            bot_arg,
        ],
        value: false.into(),
    });

    // Template
    ChronicleTemplate {
        label: Some("pick-up".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_drop_action_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start & End
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end = start + FAtom::EPSILON;

    // Name & Argument
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(DROP_ACTION).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(PACKAGE_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let package_arg = name[2];
    let loc_arg = name[3];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Action,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(name.clone()),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Conditions
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            loc_arg,
        ],
        value: true.into(),
    });
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(HOLDING_PRED).unwrap())
                .into(),
            bot_arg,
            package_arg,
        ],
        value: true.into(),
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            package_arg,
            loc_arg,
        ],
        value: true.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(HOLDING_PRED).unwrap())
                .into(),
            bot_arg,
            package_arg,
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap())
                .into(),
            bot_arg,
        ],
        value: true.into(),
    });

    // Template
    ChronicleTemplate {
        label: Some("drop".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_m_transfer_method_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    // End
    let end = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
    params.push(end.into());
    let end = FAtom::from(end);

    // Name & Argument
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(M_TRANSFER_METHOD).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(PACKAGE_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let package_arg = name[2];
    let loc_s_arg = name[3];
    let loc_e_arg = name[4];

    // Task
    let task: Vec<SAtom> = vec![
        context
            .typed_sym(context.model.get_symbol_table().id(TRANSFER_TASK).unwrap())
            .into(),
        package_arg,
        loc_e_arg,
    ];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Method,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(task),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Subtasks
    let st = create_subtask(
        context,
        c,
        prez,
        Some(&mut params),
        vec![
            context
                .typed_sym(context.model.get_symbol_table().id(GOTO_TASK).unwrap())
                .into(),
            bot_arg,
            loc_s_arg,
        ],
    );
    let prev_end = st.end;
    ch.subtasks.push(st);

    let st = create_subtask(
        context,
        c,
        prez,
        Some(&mut params),
        vec![
            context
                .typed_sym(context.model.get_symbol_table().id(PICK_UP_ACTION).unwrap())
                .into(),
            bot_arg,
            package_arg,
            loc_s_arg,
        ],
    );
    ch.constraints.push(Constraint::lt(prev_end, st.start));
    let prev_end = st.end;
    ch.subtasks.push(st);

    let st = create_subtask(
        context,
        c,
        prez,
        Some(&mut params),
        vec![
            context
                .typed_sym(context.model.get_symbol_table().id(GOTO_TASK).unwrap())
                .into(),
            bot_arg,
            loc_e_arg,
        ],
    );
    ch.constraints.push(Constraint::lt(prev_end, st.start));
    let prev_end = st.end;
    ch.subtasks.push(st);

    let st = create_subtask(
        context,
        c,
        prez,
        Some(&mut params),
        vec![
            context
                .typed_sym(context.model.get_symbol_table().id(DROP_ACTION).unwrap())
                .into(),
            bot_arg,
            package_arg,
            loc_e_arg,
        ],
    );
    ch.constraints.push(Constraint::lt(prev_end, st.start));
    ch.subtasks.push(st);

    // Template
    ChronicleTemplate {
        label: Some("m-transfer".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_m_already_transfered_method_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    // End
    let end = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
    params.push(end.into());
    let end = FAtom::from(end);

    // Name & Argument
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(
            context
                .model
                .get_symbol_table()
                .id(M_ALREADY_TRANSFERED_METHOD)
                .unwrap(),
        )
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(PACKAGE_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let package_arg = name[1];
    let loc_arg = name[2];

    // Task
    let task: Vec<SAtom> = vec![
        context
            .typed_sym(context.model.get_symbol_table().id(TRANSFER_TASK).unwrap())
            .into(),
        package_arg,
        loc_arg,
    ];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Method,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(task),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Conditions
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            package_arg,
            loc_arg,
        ],
        value: true.into(),
    });

    // Template
    ChronicleTemplate {
        label: Some("m-already-transfer".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_m_goto_method_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    // End
    let end = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
    params.push(end.into());
    let end = FAtom::from(end);

    // Name & Argument
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(M_GOTO_METHOD).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let ls_arg = name[2];
    let le_arg = name[3];

    // Task
    let task: Vec<SAtom> = vec![
        context
            .typed_sym(context.model.get_symbol_table().id(GOTO_TASK).unwrap())
            .into(),
        bot_arg,
        le_arg,
    ];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Method,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(task),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Subtasks
    let st = create_subtask(
        context,
        c,
        prez,
        Some(&mut params),
        vec![
            context
                .typed_sym(context.model.get_symbol_table().id(MOVE_ACTION).unwrap())
                .into(),
            bot_arg,
            ls_arg,
            le_arg,
        ],
    );
    ch.subtasks.push(st);

    // Template
    ChronicleTemplate {
        label: Some("m-goto".to_string()),
        parameters: params,
        chronicle: ch,
    }
}

fn get_m_already_there_method_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    // End
    let end = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
    params.push(end.into());
    let end = FAtom::from(end);

    // Name & Argument
    let mut name: Vec<SAtom> = vec![context
        .typed_sym(context.model.get_symbol_table().id(M_ALREADY_THERE_METHOD).unwrap())
        .into()];
    let args = vec![
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
        context.model.new_optional_sym_var(
            context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(),
            prez,
            c / VarType::Parameter,
        ),
    ];
    for arg in args {
        params.push(arg.into());
        name.push(arg.into());
    }
    let bot_arg = name[1];
    let loc_arg = name[2];

    // Task
    let task: Vec<SAtom> = vec![
        context
            .typed_sym(context.model.get_symbol_table().id(GOTO_TASK).unwrap())
            .into(),
        bot_arg,
        loc_arg,
    ];

    // Chronicle
    let mut ch = Chronicle {
        kind: ChronicleKind::Method,
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(task),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Conditions
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context
                .typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap())
                .into(),
            bot_arg,
            loc_arg,
        ],
        value: true.into(),
    });

    // Template
    ChronicleTemplate {
        label: Some("m-already-there".to_string()),
        parameters: params,
        chronicle: ch,
    }
}
//#endregion
