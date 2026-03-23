use clap::Parser;
use std::path::PathBuf;

use planx::{errors::*, pddl::*};

/// A simple parser for PDDL and its extension HDDL.
///
/// The parser just prints the parsed model (doamin & problem), reporting any error encountered.
#[derive(Debug, Parser)]
#[command(name = "aries-pddl", rename_all = "kebab-case")]
struct Args {
    /// If not set, will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[arg(long, short)]
    domain: Option<PathBuf>,
    /// Path to the problem file to parse.
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
        None => find_domain_of(&problem_file).title(
            "Unable to automatically find the domain file. Consider specifying the domain with the option -d/--domain",
        )?, // TODO: this erases the previous, possibly more informative, error message
    };
    let domain_file = input::Input::from_file(&domain_file)?;

    let problem_file = input::Input::from_file(&problem_file)?;
    let domain = parser::parse_pddl_domain(domain_file)?;
    let problem = parser::parse_pddl_problem(problem_file)?;

    let model = build_model(&domain, &problem)?;

    println!("{model}");

    Ok(())
}
