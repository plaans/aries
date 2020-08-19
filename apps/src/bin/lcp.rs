use anyhow::*;

use aries_planning::chronicles::FiniteProblem;

use std::path::Path;
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "lcp", rename_all = "kebab-case")]
struct Opt {
    /// File containing a JSON encoding of the finite problem to solve.
    problem: String,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let _start_time = std::time::Instant::now();

    let problem_file = Path::new(&opt.problem);
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let json = std::fs::read_to_string(problem_file)?;
    let pb: FiniteProblem<usize> = serde_json::from_str(&json)?;

    println!("{} {}", pb.origin, pb.horizon);

    Ok(())
}
