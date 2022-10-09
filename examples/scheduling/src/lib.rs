use aries_core::*;
use aries_solver::solver::search::activity::Heuristic;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Var {
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(usize, usize),
    Prec(usize, usize, usize, usize),
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
    /// Activity based search
    Activity,
    /// Variable selection based on earliest starting time + least slack
    Est,
    /// Run both Activity and Est in parallel.
    Parallel,
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "act" | "activity" => Ok(SearchStrategy::Activity),
            "est" => Ok(SearchStrategy::Est),
            "par" | "parallel" => Ok(SearchStrategy::Parallel),
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
