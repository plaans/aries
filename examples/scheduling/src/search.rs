mod conflicts;
mod greedy;

use crate::problem::Problem;
use crate::search::conflicts::ConflictBasedBrancher;
use crate::search::greedy::EstBrancher;
use aries::core::*;
use aries::solver::search::activity::Heuristic;
use aries::solver::search::combinators::{CombinatorExt, UntilFirstConflict};
use aries::solver::search::lexical::LexicalMinValue;
use aries::solver::search::Brancher;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Var {
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(u32, u32),
    Prec(u32, u32, u32, u32),
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
    LearningRate,
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lrb" | "learning-rate" => Ok(SearchStrategy::LearningRate),
            "vsids" | "activity" => Ok(SearchStrategy::Activity),
            e => Err(format!("Unrecognized option: '{e}'")),
        }
    }
}

pub struct ResourceOrderingFirst;
impl Heuristic<Var> for ResourceOrderingFirst {
    fn decision_stage(&self, _var: VarRef, label: Option<&Var>, _model: &aries::model::Model<Var>) -> u8 {
        match label {
            Some(&Var::Prec(_, _, _, _)) => 0, // a reification of (a <= b), decide in the first stage
            Some(&Var::Makespan) | Some(&Var::Start(_, _)) => 1, // delay decisions on the temporal variable to the second stage
            _ => 2,
        }
    }
}

/// Builds a solver for the given strategy.
pub fn get_solver(base: Solver, strategy: SearchStrategy, pb: &Problem) -> ParSolver {
    let first_est: Brancher<Var> = Box::new(UntilFirstConflict::new(Box::new(EstBrancher::new(pb))));

    let base_solver = Box::new(base);

    let make_solver = |s: &mut Solver, params: conflicts::Params| {
        let ema: Brancher<Var> = Box::new(ConflictBasedBrancher::with(params));
        let ema = ema.with_restarts(100, 1.2);
        let strat = first_est
            .clone_to_box()
            .and_then(ema)
            .and_then(Box::new(LexicalMinValue::new()));
        s.set_brancher_boxed(strat);
    };

    match strategy {
        SearchStrategy::Activity => ParSolver::new(base_solver, 1, |_, s| {
            make_solver(
                s,
                conflicts::Params {
                    heuristic: conflicts::Heuristic::Vsids,
                    ..Default::default()
                },
            )
        }),
        SearchStrategy::LearningRate => ParSolver::new(base_solver, 1, |_, s| {
            make_solver(
                s,
                conflicts::Params {
                    heuristic: conflicts::Heuristic::LearningRate,
                    ..Default::default()
                },
            )
        }),
    }
}
