#![allow(dead_code)]

use anyhow::*;
use aries_planning::classical::search::{plan_search, Cfg};
use aries_planning::classical::{from_chronicles, grounded_problem};
use aries_planning::write::writeplan;
use aries_planning::parsing::pddl_to_chronicles;

use std::fmt::Formatter;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use std::io;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "gg", rename_all = "kebab-case")]
struct Opt {
    /// If not set, `gg` will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<String>,
    problem: String,
    #[structopt(short = "w", default_value = "3")]
    h_weight: f32,
    #[structopt(long)]
    no_lookahead: bool,

    /// Make gg return failure with code 1 if it does not solve the problem
    #[structopt(long)]
    expect_sat: bool,

    /// Make gg return failure with code 1 if it does not prove the problem to be unsat
    #[structopt(long)]
    expect_unsat: bool,

    #[structopt(short ="p", long= "plan")]
    plan: bool,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let start_time = std::time::Instant::now();

    let mut config = Cfg::default();
    config.h_weight = opt.h_weight;
    config.use_lookahead = !opt.no_lookahead;

    let problem_file = Path::new(&opt.problem);
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(name) => PathBuf::from(&name),
        None => {
            let dir = problem_file.parent().unwrap();
            let candidate1 = dir.join("domain.pddl");
            let candidate2 = dir.parent().unwrap().join("domain.pddl");
            if candidate1.exists() {
                candidate1
            } else if candidate2.exists() {
                candidate2
            } else {
                bail!("Could not find find a corresponding 'domain.pddl' file in same or parent directory as the problem file.\
                 Consider adding it explicitly with the -d/--domain option");
            }
        }
    };

    let planwrite = opt.plan;

    let dom = std::fs::read_to_string(domain_file)?;

    let prob = std::fs::read_to_string(problem_file)?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let lifted = from_chronicles(&spec)?;

    let grounded = grounded_problem(&lifted)?;

    let symbols = &lifted.world.table;
    let search_result = plan_search(&grounded.initial_state, &grounded.operators, &grounded.goals, &config);
    let end_time = std::time::Instant::now();
    let runtime = end_time - start_time;
    let result = match search_result {
        Some(plan) => {
            println!("Got plan: {} actions", plan.len());
            println!("=============");
            for &op in &plan {
                println!("{}", symbols.format(grounded.operators.name(op)));
            }
            if planwrite{
                println!("\nEnter the path to the file where you want to write your plan");
                let mut guess = String::new();
                io::stdin()
                    .read_line(&mut guess)
                    .expect("Failed to read line");
                writeplan(guess,&plan,&grounded,symbols);
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
            self.cost.map_or_else(|| "_".to_string(), |cost| format!("{}", cost)),
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

