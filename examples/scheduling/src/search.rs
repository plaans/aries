mod greedy;

use crate::problem::{Encoding, OperationId};
use crate::search::SearchStrategy::Custom;
use crate::search::greedy::EstBrancher;
use aries::prelude::*;
use aries::solver::search::combinators::{CombinatorExt, UntilFirstConflict};
use aries::solver::search::conflicts::{ConflictBasedBrancher, ImpactMeasure};
use aries::solver::search::lexical::Lexical;
use aries::solver::search::{Brancher, SearchControl, conflicts};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Var {
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(OperationId),
    /// Variable representing an oredring between two operations
    Prec(OperationId, OperationId),
    /// Variable encoding whether an operation is part of the solution
    Presence(OperationId),
}

impl std::fmt::Display for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type Model = aries::model::Model<Var>;
pub type Solver = aries::solver::Solver<Var>;

/// Variants of the search strategy
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SearchStrategy {
    /// Default strategy, typically an optimization oriente solution-guidance qith LRB
    Default,
    /// Custom strategy, parsed from the command line arguments.
    Custom(String),
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(SearchStrategy::Default),
            _ => Ok(Custom(s.to_string())),
        }
    }
}

#[derive(Copy, Clone)]
enum Mode {
    Stable,
    Focused,
}

#[derive(Clone)]
struct Strat {
    mode: Mode,
    params: conflicts::Params,
}

/// Builds a solver for the given strategy.
pub fn get_solver(mut base_solver: Solver, strategy: &SearchStrategy, pb: &Encoding) -> Solver {
    let mut load_conf = |conf: &str| -> Strat {
        let mut mode = Mode::Stable;
        let mut params = conflicts::Params {
            heuristic: conflicts::Heuristic::LearningRate,
            active: conflicts::ActiveLiterals::Reasoned,
            impact_measure: ImpactMeasure::LBD,
            ..Default::default()
        };
        for opt in conf.split(':') {
            if params.configure(opt) {
                // handled
                continue;
            }
            match opt {
                "stable" => mode = Mode::Stable,
                "focused" => mode = Mode::Focused,
                x if x.starts_with("+lbd") => {
                    let lvl = x.strip_prefix("+lbd").unwrap().parse().unwrap();
                    base_solver.reasoners.sat.clauses.params.locked_lbd_level = lvl;
                }
                "" => {} // ignore
                _ => panic!("Unsupported option: {opt}"),
            }
        }
        Strat { mode, params }
    };

    let conf = match strategy {
        SearchStrategy::Default => "stable:+sol",
        SearchStrategy::Custom(conf) => conf.as_str(),
    };
    let strat = load_conf(conf);

    let decision_lits: Vec<Lit> = base_solver
        .model
        .state
        .variables()
        .filter_map(|v| match base_solver.model.shape.labels.get(v) {
            Some(&Var::Prec(_, _)) => Some(v.geq(1)),
            Some(&Var::Presence(_)) => Some(v.geq(1)),
            _ => None,
        })
        .collect();

    // creates a brancher for a given strategy
    let build_brancher = |strat: Strat| {
        let brancher: Brancher<Var> = Box::new(ConflictBasedBrancher::with(decision_lits.clone(), strat.params));
        let (restart_period, restart_update) = match strat.mode {
            Mode::Stable => (2000, 1.2), // stable: few restarts
            Mode::Focused => (800, 1.0), // focused: always aggressive restarts
        };
        brancher.with_restarts(restart_period, restart_update)
    };

    // Bootstraping branching strategy: a greedy EST strategy to bootstrap the search
    let first_est: Brancher<Var> = Box::new(UntilFirstConflict::new(Box::new(EstBrancher::new(pb))));
    // main brancher (after first conflict and as long binary vars are not set): Conflict baed search (LRB, ...)
    let main_brancher = build_brancher(strat);
    // add last strategy to ensure that all variables are bound (main strategy only takes care of bineary decision variables)
    let final_brancher = Lexical::with_min().clone_to_box();
    let brancher = first_est.and_then(main_brancher).and_then(final_brancher);
    base_solver.set_brancher_boxed(brancher);
    base_solver
}
