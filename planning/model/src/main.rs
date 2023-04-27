mod fluents;
mod pddl;
mod sexpr;
mod source;
mod types;

use anyhow::*;
use clap::Parser;
use std::path::PathBuf;

use crate::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem};
use aries::utils::input::Input;

/// A simple parser for PDDL and its extension HDDL.
/// Its main intended usage is to facilitate automated testing of the parser in a CI environment.
// #[derive(Debug, StructOpt)]
// #[structopt(name = "pddl", rename_all = "kebab-case")]
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opt {
    /// If not set, will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    problem: PathBuf,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::parse();

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

    // let _chronicles = pddl_to_chronicles(&dom, &prob)?;

    Ok(())
}
