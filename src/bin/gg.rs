#![allow(dead_code)]

use aries::planning::classical::search::{plan_search, Cfg};
use aries::planning::classical::{from_chronicles, grounded_problem};
use aries::planning::parsing::pddl_to_chronicles;
use serde::export::Formatter;
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "gg", rename_all = "kebab-case")]
struct Opt {
    domain: String,
    problem: String,
    #[structopt(short = "w", default_value = "3")]
    h_weight: u32,
    #[structopt(long)]
    no_lookahead: bool,

    /// Make gg return failure with code 1 if it does not solve the problem
    #[structopt(long)]
    expect_sat: bool,

    /// Make gg return failure with code 1 if it does not prove the problem to be unsat
    #[structopt(long)]
    expect_unsat: bool,
}

fn main() -> Result<(), String> {
    let opt: Opt = Opt::from_args();
    let start_time = std::time::Instant::now();

    let mut config = Cfg::default();
    config.h_weight = opt.h_weight as u64;
    config.use_lookahead = !opt.no_lookahead;

    let dom = std::fs::read_to_string(opt.domain).map_err(|o| format!("{}", o))?;

    let prob = std::fs::read_to_string(opt.problem).map_err(|o| format!("{}", o))?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let lifted = from_chronicles(&spec)?;

    let grounded = grounded_problem(&lifted)?;

    let symbols = &lifted.world.table;
    let search_result = plan_search(
        &grounded.initial_state,
        &grounded.operators,
        &grounded.goals,
        &config,
    );
    let end_time = std::time::Instant::now();
    let runtime = end_time - start_time;
    let result = match search_result {
        Some(plan) => {
            println!("Got plan: {} actions", plan.len());
            println!("=============");
            for &op in &plan {
                println!("{}", symbols.format(grounded.operators.name(op)));
            }
            SolverResult {
                status: Status::SUCCESS,
                solution: Some(Solution::SAT),
                cost: Some(plan.len() as f64),
                runtime,
            }
        }
        None => SolverResult {
            status: Status::SUCCESS,
            solution: Some(Solution::UNSAT),
            cost: None,
            runtime,
        },
    };

    println!("{}", result);
    if opt.expect_sat && !result.proved_sat() {
        std::process::exit(1);
    }
    if opt.expect_unsat && result.solution != Some(Solution::UNSAT) {
        std::process::exit(1);
    }
    Ok(())
}

struct SolverResult {
    status: Status,
    solution: Option<Solution>,
    cost: Option<f64>,
    runtime: std::time::Duration,
}
impl SolverResult {
    pub fn proved_sat(&self) -> bool {
        match self.solution {
            Some(Solution::SAT) => true,
            Some(Solution::OPTIMAL) => true,
            _ => false,
        }
    }
}
impl std::fmt::Display for SolverResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[summary] status:{} solution:{} cost:{} runtime:{}ms",
            match self.status {
                Status::SUCCESS => "SUCCESS",
                Status::TIMEOUT => "TIMEOUT",
                Status::CRASH => "CRASH",
            },
            match self.solution {
                Some(Solution::SAT) => "SAT",
                Some(Solution::UNSAT) => "UNSAT",
                Some(Solution::OPTIMAL) => "OPTIMAL",
                None => "_",
            },
            self.cost
                .map_or_else(|| "_".to_string(), |cost| format!("{}", cost)),
            self.runtime.as_millis()
        )
    }
}

// TODO: either generalize in the crate or drop
//       when doing so, also remove the clippy:allow at the top of this file
enum Status {
    SUCCESS,
    TIMEOUT,
    CRASH,
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum Solution {
    UNSAT,
    SAT,
    OPTIMAL,
}
