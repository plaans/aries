mod greedy;

use crate::problem::{Encoding, OperationId};
use crate::search::greedy::EstBrancher;
use aries::core::*;
use aries::model::extensions::Shaped;
use aries::solver::search::activity::Heuristic;
use aries::solver::search::combinators::{CombinatorExt, UntilFirstConflict};
use aries::solver::search::conflicts::{ActiveLiterals, ConflictBasedBrancher, Params, ValueSelection};
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
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum SearchStrategy {
    /// greedy earliest-starting-time then VSIDS with solution guidance
    Activity,
    /// greedy earliest-starting-time then LRB with solution guidance
    /// Boolean parameter indicates whether we should prefer the value of the last solution or the opposite
    LearningRate(bool),
    Parallel,
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lrb" | "lrb+" | "learning-rate" => Ok(SearchStrategy::LearningRate(true)),
            "lrb-" | "learning-rate-neg" => Ok(SearchStrategy::LearningRate(false)),
            "vsids" | "activity" => Ok(SearchStrategy::Activity),
            "par" | "parallel" => Ok(SearchStrategy::Parallel),
            e => Err(format!("Unrecognized option: '{e}'")),
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
pub fn get_solver(base: Solver, strategy: SearchStrategy, pb: &Encoding) -> ParSolver {
    let first_est: Brancher<Var> = Box::new(UntilFirstConflict::new(Box::new(EstBrancher::new(pb))));

    let base_solver = Box::new(base);

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

    let vsids = conflicts::Params {
        heuristic: conflicts::Heuristic::Vsids,
        active: ActiveLiterals::Reasoned,
        value_selection: ValueSelection::Sol,
    };
    let lrb_sol = conflicts::Params {
        heuristic: conflicts::Heuristic::LearningRate,
        active: ActiveLiterals::Reasoned,
        value_selection: ValueSelection::Sol,
    };
    let lrb_not_sol = conflicts::Params {
        heuristic: conflicts::Heuristic::LearningRate,
        active: ActiveLiterals::Reasoned,
        value_selection: ValueSelection::NotSol,
    };

    let strats: &[Params] = match strategy {
        SearchStrategy::Activity => &[vsids],
        SearchStrategy::LearningRate(true) => &[lrb_sol],
        SearchStrategy::LearningRate(false) => &[lrb_not_sol],
        SearchStrategy::Parallel => &[lrb_sol, lrb_not_sol],
    };

    ParSolver::new(base_solver, strats.len(), |i, s| make_solver(s, strats[i]))
}
