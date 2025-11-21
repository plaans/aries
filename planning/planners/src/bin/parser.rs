use anyhow::*;
use std::path::PathBuf;
use structopt::StructOpt;

use aries::utils::input::Input;
use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem};
use aries_planning::parsing::pddl_to_chronicles;

/// A simple parser for PDDL and its extension HDDL.
/// Its main intended usage is to facilitate automated testing of the parser in a CI environment.
#[derive(Debug, StructOpt)]
#[structopt(name = "aries-pddl-parser", rename_all = "kebab-case")]
struct Opt {
    /// If not set, will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    problem: PathBuf,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

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
    println!("==== Domain ====\n{}", &dom);

    let prob = parse_pddl_problem(prob)?;
    println!("==== Problem ====\n{}", &prob);

    let _chronicles = pddl_to_chronicles(&dom, &prob, false)?;

    Ok(())
}
