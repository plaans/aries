use clap::Parser;
use std::path::PathBuf;

use aries_planning_model::{errors::*, pddl::*};

/// A simple parser for PDDL and its extension HDDL.
/// Its main intended usage is to facilitate automated testing of the parser in a CI environment.
#[derive(Debug, Parser)]
#[command(name = "aries-pddl", rename_all = "kebab-case")]
struct Args {
    /// If not set, will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[arg(long, short)]
    domain: Option<PathBuf>,
    problem: PathBuf,
}

fn main() -> Res<()> {
    let opt = Args::parse();

    let problem_file = &opt.problem;
    if !problem_file.exists() {
        return Err(Message::error(format!(
            "Problem file {} does not exist",
            problem_file.display()
        )));
    }

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(name) => name,
        None => find_domain_of(&problem_file).ctx("Consider specifying the domain with the option -d/--domain")?,
    };
    let domain_file = input::Input::from_file(&domain_file)?;

    let problem_file = input::Input::from_file(&problem_file)?;
    let domain = parser::parse_pddl_domain(domain_file)?;
    let problem = parser::parse_pddl_problem(problem_file)?;

    let _model = convert::build_model(&domain, &problem)?;

    Ok(())
}
