mod activity;
pub mod combinators;
mod ema;
mod fds;
mod greedy;
mod lexical;

use crate::problem::Problem;
use crate::search::combinators::{AndThen, Brancher, UntilFirstConflict, WithGeomRestart};
use crate::search::ema::EMABrancher;
use crate::search::fds::FDSBrancher;
use crate::search::greedy::EstBrancher;
use crate::search::lexical::LexicalMinValue;
use aries_core::*;
use aries_solver::solver::search::activity::{ActivityBrancher, Heuristic};
use combinators::CombExt;
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
        write!(f, "{:?}", self)
    }
}

pub type Model = aries_model::Model<Var>;
pub type Solver = aries_solver::solver::Solver<Var>;
pub type ParSolver = aries_solver::parallel_solver::ParSolver<Var>;

/// Search strategies that can be added to the solver.
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum SearchStrategy {
    VSIDS,
    /// Activity based search with solution guidance
    Activity,
    /// Variable selection based on earliest starting time + least slack
    Est,
    /// Failure directed search
    Fds,
    /// Solution guided: first runs Est strategy until an initial solution is found and tehn switches to activity based search
    Sol,
    /// Run both Activity and Est in parallel.
    Parallel,
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "act" | "activity" => Ok(SearchStrategy::Activity),
            "est" | "earliest-start" => Ok(SearchStrategy::Est),
            "fds" | "failure-directed" => Ok(SearchStrategy::Fds),
            "sol" | "solution-guided" => Ok(SearchStrategy::Sol),
            "par" | "parallel" => Ok(SearchStrategy::Parallel),
            "vsids" => Ok(SearchStrategy::VSIDS),
            e => Err(format!("Unrecognized option: '{}'", e)),
        }
    }
}

pub struct ResourceOrderingFirst;
impl Heuristic<Var> for ResourceOrderingFirst {
    fn decision_stage(&self, _var: VarRef, label: Option<&Var>, _model: &aries_model::Model<Var>) -> u8 {
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
    let make_vsids = |s: &mut Solver| s.set_brancher(activity::ActivityBrancher::new());
    let make_act = |s: &mut Solver| s.set_brancher(ActivityBrancher::new_with_heuristic(ResourceOrderingFirst));
    let make_est = |s: &mut Solver| s.set_brancher(EstBrancher::new(pb));

    let make_fds = |s: &mut Solver| {
        let est = Box::new(EstBrancher::new(pb));
        let first_est = Box::new(UntilFirstConflict::new(est));
        let fds = Box::new(FDSBrancher::new());
        let fds = Box::new(WithGeomRestart::new(100, 1.5, fds));
        let strat = AndThen::new(first_est, fds);
        s.set_brancher(strat);
    };

    let make_sol = |s: &mut Solver| {
        let ema: Brancher<Var> = Box::new(EMABrancher::new());
        let ema = ema.with_restarts(100, 1.2);
        let strat = first_est
            .clone_to_box()
            .and_then(ema)
            .and_then(Box::new(LexicalMinValue::new()));
        s.set_brancher_boxed(strat);
    };

    match strategy {
        SearchStrategy::VSIDS => ParSolver::new(base_solver, 1, |_, s| make_vsids(s)),
        SearchStrategy::Activity => ParSolver::new(base_solver, 1, |_, s| make_act(s)),
        SearchStrategy::Est => ParSolver::new(base_solver, 1, |_, s| make_est(s)),
        SearchStrategy::Fds => ParSolver::new(base_solver, 1, |_, s| make_fds(s)),
        SearchStrategy::Sol => ParSolver::new(base_solver, 1, |_, s| make_sol(s)),
        SearchStrategy::Parallel => ParSolver::new(base_solver, 2, |id, s| match id {
            0 => make_sol(s),
            // 1 => make_fds(s),
            1 => make_vsids(s),
            // 2 => make_fds(s),
            _ => unreachable!(),
        }),
    }
}
