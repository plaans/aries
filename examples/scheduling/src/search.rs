use crate::problem::{Op, Problem};
use aries_backtrack::{Backtrack, DecLvl};
use aries_core::*;
use aries_model::extensions::AssignmentExt;
use aries_solver::solver::search::activity::{ActivityBrancher, Heuristic};
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
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

#[derive(Clone)]
pub struct EstBrancher {
    pb: Problem,
    saved: DecLvl,
}

impl EstBrancher {
    pub fn new(pb: &Problem) -> Self {
        EstBrancher {
            pb: pb.clone(),
            saved: DecLvl::ROOT,
        }
    }
}

impl SearchControl<Var> for EstBrancher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        let active_tasks = {
            self.pb
                .operations()
                .iter()
                .copied()
                .filter_map(|Op { job, op_id, .. }| {
                    let v = model.shape.get_variable(&Var::Start(job, op_id)).unwrap();
                    let (lb, ub) = model.domain_of(v);
                    if lb < ub {
                        Some((v, lb, ub))
                    } else {
                        None
                    }
                })
        };
        // among the task with the smallest "earliest starting time (est)" pick the one that has the least slack
        let best = active_tasks.min_by_key(|(_var, est, lst)| (*est, *lst));

        // decision is to set the start time to the selected task to the smallest possible value.
        // if no task was selected, it means that they are all instantiated and we have a complete schedule
        best.map(|(var, est, _)| Decision::SetLiteral(Lit::leq(var, est)))
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for EstBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}

/// Builds a solver for the given strategy.
pub fn get_solver(base: Solver, strategy: SearchStrategy, est_brancher: EstBrancher) -> ParSolver {
    let base_solver = Box::new(base);
    let make_act = |s: &mut Solver| s.set_brancher(ActivityBrancher::new_with_heuristic(ResourceOrderingFirst));
    let make_est = |s: &mut Solver| s.set_brancher(est_brancher.clone());
    match strategy {
        SearchStrategy::Activity => ParSolver::new(base_solver, 1, |_, s| make_act(s)),
        SearchStrategy::Est => ParSolver::new(base_solver, 1, |_, s| make_est(s)),
        SearchStrategy::Parallel => ParSolver::new(base_solver, 2, |id, s| match id {
            0 => make_act(s),
            1 => make_est(s),
            _ => unreachable!(),
        }),
    }
}
