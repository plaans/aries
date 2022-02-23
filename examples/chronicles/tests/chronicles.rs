use aries_core::*;
use aries_planning::chronicles::*;
use aries_planning::chronicles::constraints::Constraint;
use aries_planners::encode::{encode, populate_with_template_instances};
use aries_planners::fmt::{format_pddl_plan};
use aries_planners::Solver;
use aries_model::extensions::{SavedAssignment, Shaped};
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_solver::parallel_solver::ParSolver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};
use aries_utils::input::{Sym};
use std::sync::Arc;
use std::time::Instant;

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
// My durative actions
static MOVE_DUR_ACTION: &str = "move";
// My tasks: None
// My methods: None
// My functions: None

#[test]
fn test_no_htn_problem() {
    let mut pb = get_no_htn_problem();
    run_problem(&mut pb);
}

//#region Main & Solver
fn run_problem(problem: &mut Problem) {
    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(problem);
    println!("==========================");

    let max_depth = u32::MAX;
    let min_depth = 0;

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
        populate_with_template_instances(&mut pb, &problem, |_| Some(n)).unwrap();
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        let result = solve(&pb);
        println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
        if let Some(x) = result {
            // println!("{}", format_partial_plan(&pb, &x)?);
            println!("  Solution found");
            let plan = format_pddl_plan(&pb, &x).unwrap();
            println!("{}", plan);
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

fn solve(pb: &FiniteProblem) -> Option<std::sync::Arc<SavedAssignment>> {
    let solver = init_solver(pb);
    let mut solver = ParSolver::new(solver, 1, |_, _| {});

    let found_plan = solver.solve().unwrap();

    if let Some(solution) = found_plan {
        solver.print_stats();
        Some(solution)
    } else {
        None
    }
}
//#endregion

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
    let pb = create_problem_chronicle_instance(&mut context);

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
                Type::Bool
            ]
        },
        StateFun {
            sym: symbol_table.id(HOLDING_PRED).unwrap(),
            tpe: vec![
                Type::Sym(symbol_table.types.id_of(BOT_TYPE).unwrap()),
                Type::Sym(symbol_table.types.id_of(PACKAGE_TYPE).unwrap()),
                Type::Bool
            ]
        },
        StateFun {
            sym: symbol_table.id(EMPTY).unwrap(),
            tpe: vec![
                Type::Sym(symbol_table.types.id_of(BOT_TYPE).unwrap()),
                Type::Bool
            ]
        }
    ]
}

fn create_problem_chronicle_instance(context: &mut Ctx) -> ChronicleInstance {
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
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(PACKAGE_OBJ).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(LOC2_OBJ).unwrap()).into()
        ],
        value: true.into()
    });

    // Creation of the initial state
    pb.effects.push(Effect {
        transition_start: pb.start,
        persistence_start: pb.start,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(ROBOT_OBJ).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(LOC1_OBJ).unwrap()).into()
        ],
        value: true.into(),
    });
    pb.effects.push(Effect {
        transition_start: pb.start,
        persistence_start: pb.start,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(PACKAGE_OBJ).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(LOC1_OBJ).unwrap()).into()
        ],
        value: true.into(),
    });
    pb.effects.push(Effect {
        transition_start: pb.start,
        persistence_start: pb.start,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap()).into(),
            context.typed_sym(context.model.get_symbol_table().id(ROBOT_OBJ).unwrap()).into()
        ],
        value: true.into(),
    });

    // Instantiation of the problem
    ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: pb
    }
}

fn get_move_duractive_action_template(templates_len: usize, context: &mut Ctx) -> ChronicleTemplate {
    let c = Container::Template(templates_len);
    let mut params: Vec<Variable> = vec![];

    // Presence
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    // Start
    let start = context.model.new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    
    // End
    let end = context.model.new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
    params.push(end.into());
    let end = end.into();

    // Name & Arguments
    let mut name: Vec<SAtom> = vec![
        context.typed_sym(context.model.get_symbol_table().id(MOVE_DUR_ACTION).unwrap()).into()
    ];
    let args = vec![
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(), prez, c / VarType::Parameter),
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(), prez, c / VarType::Parameter),
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(), prez, c / VarType::Parameter),
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
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            bot_arg,
            from_arg
        ],
        value: true.into()
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.start + FAtom::EPSILON,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            bot_arg,
            from_arg
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.end,
        persistence_start: ch.end + FAtom::EPSILON,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            bot_arg,
            to_arg
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
    let start = context.model.new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end = start + FAtom::EPSILON;

    // Name & Argument
    let mut name: Vec<SAtom> = vec![
        context.typed_sym(context.model.get_symbol_table().id(PICK_UP_ACTION).unwrap()).into()
    ];
    let args = vec![
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(), prez, c / VarType::Parameter),
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(PACKAGE_TYPE).unwrap(), prez, c / VarType::Parameter),
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(), prez, c / VarType::Parameter),
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
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            bot_arg,
            loc_arg
        ],
        value: true.into()
    });
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            package_arg,
            loc_arg
        ],
        value: true.into()
    });
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap()).into(),
            bot_arg
        ],
        value: true.into()
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            package_arg,
            loc_arg
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(HOLDING_PRED).unwrap()).into(),
            bot_arg,
            package_arg
        ],
        value: true.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap()).into(),
            bot_arg
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
    let start = context.model.new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end = start + FAtom::EPSILON;

    // Name & Argument
    let mut name: Vec<SAtom> = vec![
        context.typed_sym(context.model.get_symbol_table().id(DROP_ACTION).unwrap()).into()
    ];
    let args = vec![
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(BOT_TYPE).unwrap(), prez, c / VarType::Parameter),
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(PACKAGE_TYPE).unwrap(), prez, c / VarType::Parameter),
        context.model.new_optional_sym_var(context.model.get_symbol_table().types.id_of(LOCATION_TYPE).unwrap(), prez, c / VarType::Parameter),
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
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            bot_arg,
            loc_arg
        ],
        value: true.into()
    });
    ch.conditions.push(Condition {
        start: ch.start,
        end: ch.start,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(HOLDING_PRED).unwrap()).into(),
            bot_arg,
            package_arg
        ],
        value: true.into()
    });

    // Effects
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(ON_PRED).unwrap()).into(),
            package_arg,
            loc_arg
        ],
        value: true.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(HOLDING_PRED).unwrap()).into(),
            bot_arg,
            package_arg
        ],
        value: false.into(),
    });
    ch.effects.push(Effect {
        transition_start: ch.start,
        persistence_start: ch.end,
        state_var: vec![
            context.typed_sym(context.model.get_symbol_table().id(EMPTY).unwrap()).into(),
            bot_arg
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
