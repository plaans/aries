mod greedy;

use crate::problem::{Encoding, OperationId};
use crate::search::greedy::EstBrancher;
use crate::search::SearchStrategy::Custom;
use aries::core::*;
use aries::model::extensions::Shaped;
use aries::solver::search::activity::Heuristic;
use aries::solver::search::combinators::{CombinatorExt, UntilFirstConflict};
use aries::solver::search::conflicts::{ConflictBasedBrancher, Params};
use aries::solver::search::lexical::LexicalMinValue;
use aries::solver::search::{conflicts, Brancher};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Var {
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(OperationId),
    Prec(OperationId, OperationId),
    Presence(OperationId),
    // a variable used to encode the problem but not necessary to reconstruct the solution.
    Intermediate,
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
    /// greedy earliest-starting-time then VSIDS with solution guidance
    Activity,
    /// greedy earliest-starting-time then LRB with solution guidance
    LearningRate,
    /// Custom strategy, parsed from the command line arguments.
    Custom(String),
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lrb" | "lrb+" | "learning-rate" => Ok(SearchStrategy::LearningRate),
            "vsids" | "activity" => Ok(SearchStrategy::Activity),
            _ => Ok(Custom(s.to_string())),
        }
    }
}

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

/// Builds a solver for the given strategy.
pub fn get_solver(base: Solver, strategy: &SearchStrategy, pb: &Encoding) -> ParSolver {
    let first_est: Brancher<Var> = Box::new(UntilFirstConflict::new(Box::new(EstBrancher::new(pb))));

    let mut base_solver = Box::new(base);

    let mut load_conf = |conf: &str| -> conflicts::Params {
        let mut params = conflicts::Params::default();
        params.heuristic = conflicts::Heuristic::LearningRate;
        params.active = conflicts::ActiveLiterals::Reasoned;
        for opt in conf.split(':') {
            match opt {
                "+phase" | "+p" => params.value_selection.phase_saving = true,
                "-phase" | "-p" => params.value_selection.phase_saving = false,
                "+sol" | "+s" => params.value_selection.solution_guidance = true,
                "-sol" | "-s" => params.value_selection.solution_guidance = false,
                "+neg" => params.value_selection.take_opposite = true,
                "-neg" => params.value_selection.take_opposite = false,
                "+longest" => params.value_selection.save_phase_on_longest = true,
                "-longest" => params.value_selection.save_phase_on_longest = false,
                "+lrb" => {
                    params.heuristic = conflicts::Heuristic::LearningRate;
                    params.active = conflicts::ActiveLiterals::Reasoned;
                }
                "+vsids" => {
                    params.heuristic = conflicts::Heuristic::Vsids;
                }
                x if x.starts_with("+lbd") => {
                    let lvl = x.strip_prefix("+lbd").unwrap().parse().unwrap();
                    base_solver.reasoners.sat.clauses.params.locked_lbd_level = lvl;
                }

                "" => {} // ignore
                _ => panic!("Unsupported option: {opt}"),
            }
        }
        params
    };

    let strats: &[Params] = match strategy {
        SearchStrategy::Activity => &[load_conf("+vsids")],
        SearchStrategy::LearningRate => &[load_conf("+lrb")],
        SearchStrategy::Custom(conf) => &[load_conf(&conf)],
    };

    let make_solver = |s: &mut Solver, params: conflicts::Params| {
        let decision_lits: Vec<Lit> = s
            .model
            .state
            .variables()
            .filter_map(|v| match s.model.get_label(v) {
                Some(&Var::Prec(_, _)) => Some(v.geq(1)),
                Some(&Var::Presence(_)) => Some(v.geq(1)),
                _ => None,
            })
            .collect();
        let ema: Brancher<Var> = Box::new(ConflictBasedBrancher::with(decision_lits, params));
        let ema = ema.with_restarts(100, 1.2);
        let strat = first_est
            .clone_to_box()
            .and_then(ema)
            .and_then(Box::new(LexicalMinValue::new()));
        s.set_brancher_boxed(strat);
    };

    ParSolver::new(base_solver, strats.len(), |i, s| make_solver(s, strats[i]))
}
