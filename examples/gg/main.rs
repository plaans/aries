use anyhow::*;
use aries_planning::classical::search::{plan_search, Cfg};
use aries_planning::classical::{from_chronicles, grounded_problem};
use aries_planning::parsing::pddl_to_chronicles;

use std::fmt::Formatter;
use std::path::PathBuf;
use structopt::StructOpt;

use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem};
use aries_utils::input::Input;
use std::fs::File;
use std::io::Write;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "gg", rename_all = "kebab-case")]
struct Opt {
    /// If not set, `gg` will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    problem: PathBuf,
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

    /// If a plan is found, it will be written to the indicated file.
    #[structopt(short = "p", long = "plan")]
    plan_file: Option<String>,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let start_time = std::time::Instant::now();

    let config = Cfg {
        h_weight: opt.h_weight,
        use_lookahead: !opt.no_lookahead,
    };

    let problem_file = &opt.problem;
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(name) => name,
        None => find_domain_of(&problem_file).context("Consider specifying the domain with the option -d/--domain")?,
    };

    let dom = Input::from_file(&domain_file)?;
    let prob = Input::from_file(&problem_file)?;

    let dom = parse_pddl_domain(dom)?;
    let prob = parse_pddl_problem(prob)?;
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
            if let Some(plan_file) = opt.plan_file {
                let mut output = File::create(&plan_file)
                    .with_context(|| format!("Option -p failed to create file {}", &plan_file))?;
                for &op in &plan {
                    writeln!(output, "{}", symbols.format(grounded.operators.name(op)))
                        .with_context(|| "Error while writing plan.")?;
                }
            }
            SolverResult {
                solution: Some(Solution::Sat),
                cost: Some(plan.len() as f64),
                runtime,
            }
        }
        None => SolverResult {
            solution: Some(Solution::Unsat),
            cost: None,
            runtime,
        },
    };

    println!("{result}");
    if opt.expect_sat && !result.proved_sat() {
        std::process::exit(1);
    }
    if opt.expect_unsat && result.solution != Some(Solution::Unsat) {
        std::process::exit(1);
    }
    Ok(())
}

struct SolverResult {
    solution: Option<Solution>,
    cost: Option<f64>,
    runtime: std::time::Duration,
}
impl SolverResult {
    pub fn proved_sat(&self) -> bool {
        self.solution == Some(Solution::Sat)
    }
}
impl std::fmt::Display for SolverResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[summary] solution:{} cost:{} runtime:{}ms",
            match self.solution {
                Some(Solution::Sat) => "SAT",
                Some(Solution::Unsat) => "UNSAT",
                None => "_",
            },
            self.cost.map_or_else(|| "_".to_string(), |cost| format!("{cost}")),
            self.runtime.as_millis()
        )
    }
}

#[derive(Eq, PartialEq)]
enum Solution {
    Unsat,
    Sat,
}
