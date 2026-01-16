mod greedy;

use crate::problem::{Encoding, OperationId};
use crate::search::greedy::EstBrancher;
use crate::search::SearchStrategy::Custom;
use aries::core::*;
use aries::model::extensions::Shaped;
use aries::solver::search::activity::Heuristic;
use aries::solver::search::combinators::{CombinatorExt, RoundRobin, UntilFirstConflict};
use aries::solver::search::conflicts::{ConflictBasedBrancher, ImpactMeasure};
use aries::solver::search::lexical::Lexical;
use aries::solver::search::{conflicts, Brancher, SearchControl};
use itertools::Itertools;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Var {
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(OperationId),
    Prec(OperationId, OperationId),
    Presence(OperationId),
}

impl std::fmt::Display for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type Model = aries::model::Model<Var>;
pub type Solver = aries::solver::Solver<Var>;
pub type ParSolver = aries::solver::parallel::ParSolver<Var>;

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

#[allow(unused)]
pub struct ResourceOrderingFirst;
impl Heuristic<Var> for ResourceOrderingFirst {
    fn decision_stage(&self, _var: VarRef, label: Option<&Var>, _model: &aries::model::Model<Var>) -> u8 {
        match label {
            Some(&Var::Prec(_, _)) => 0,  // a reification of (a <= b), decide in the first stage
            Some(&Var::Presence(_)) => 0, // presence of an alternative
            Some(&Var::Makespan) | Some(&Var::Start(_)) => 1, // delay decisions on the temporal variable to the second stage
            _ => 2,
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
pub fn get_solver(base: Solver, strategy: &SearchStrategy, pb: &Encoding, num_threads: usize) -> ParSolver {
    let mut base_solver = Box::new(base);

    let load_conf = |conf: &str| -> Strat {
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
    let all_strats = conf.split('/').map(load_conf).collect_vec();

    let decision_lits: Vec<Lit> = base_solver
        .model
        .state
        .variables()
        .filter_map(|v| match base_solver.model.get_label(v) {
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

    // build a brancher that alternates between the proposed strategies
    let round_robin = |mut branchers: Vec<Brancher<Var>>| {
        assert_ne!(branchers.len(), 0);
        if branchers.len() == 1 {
            branchers.remove(0)
        } else {
            RoundRobin::new(10000, 1.1, branchers).clone_to_box()
        }
    };

    ParSolver::new(base_solver, num_threads, |thread_id, s| {
        // select the strategies that must be run on this thread
        let strats = all_strats
            .iter()
            .enumerate()
            .filter_map(|(i, strat)| {
                if i % num_threads == thread_id {
                    Some(strat.clone())
                } else {
                    None
                }
            })
            .collect_vec();

        // conflict based search, possibly alternating between several strategies
        let branchers = strats.into_iter().map(build_brancher).collect_vec();
        let brancher = round_robin(branchers);

        // search strategy. For the first one simply add a greedy EST strategy to bootstrap the search
        let brancher = if thread_id == 0 {
            let first_est: Brancher<Var> = Box::new(UntilFirstConflict::new(Box::new(EstBrancher::new(pb))));
            first_est.and_then(brancher)
        } else {
            brancher
        };
        // add last strategy to ensure that all variables are bound (main strategy only takes care of bineary decision variables)
        let brancher = brancher.and_then(Lexical::with_min().clone_to_box());
        s.set_brancher_boxed(brancher)
    })
}
