use std::fs;
use std::path::PathBuf;

// use clap::value_parser;
use clap::Parser;

use crate::aries::solver::Solver;
use crate::fzn::output;
use crate::fzn::output::make_output;
use crate::fzn::parser::parse_model;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Aries solver for flatzinc models.",
    long_about = None
)]
pub struct Args {
    /*
    /// Report all solutions
    #[arg(short, long)]
    pub all_solutions: bool,
    */

    /*
    /// Stop after after N solutions. Not implemented.
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 1,
        value_parser = value_parser!(u32).range(1..),
    )]
    pub nb_solutions: u32,
    */
    /// Report intermediate solutions.
    #[arg(short, long)]
    pub intermediate: bool,

    /*
    /// Ignore search annotations. Not implemented.
    #[arg(short, long)]
    pub free_search: bool,
    */

    /*
    /// Print search statistics. Not implemented.
    #[arg(short, long)]
    pub statistics: bool,
    */

    /*
    /// Use verbose output. Not implemented.
    #[arg(short, long)]
    pub verbose: bool,
    */

    /*
    /// Run with N parallel threads. Not implemented.
    #[arg(
        short = 'p',
        long,
        value_name = "N",
        default_value_t = 1,
        value_parser = value_parser!(u32).range(1..),
    )]
    pub nb_threads: u32,
    */

    /*
    /// Set random seed. Not implemented.
    #[arg(short, long, value_name = "SEED")]
    pub random_seed: Option<u64>,
    */

    /*
    /// Set time limit in milliseconds. Not implemented.
    #[arg(short, long, value_name = "MS")]
    pub time: Option<u64>,
    */
    /// Flatzinc model. Not implemented.
    #[arg(value_name = "FILE")]
    pub model: PathBuf,
}

pub fn parse_args() -> Args {
    Args::parse()
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let content = fs::read_to_string(&args.model)?;
    let model = parse_model(content)?;
    let solver = Solver::new(model);
    if args.intermediate {
        let mut unsat = true;
        let f = |a| {
            unsat = false;
            println!("{}", make_output(Some(a)))
        };
        solver.solve_with(f)?;
        if unsat {
            println!("{}", make_output(None))
        }
    } else {
        let result = solver.solve()?;
        let output = make_output(result);
        println!("{output}");
    }
    if solver.fzn_model().solve_item().is_optimize() {
        println!("{}", output::END_OF_SEARCH);
    }
    Ok(())
}
