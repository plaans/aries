//! Command line interface.

use std::fs;
use std::path::PathBuf;

// use clap::value_parser;
use clap::Parser;

use crate::aries::Solver;
use crate::fzn::parser::parse_model;
use crate::fzn::solution::make_output_flow;
use crate::fzn::Fzn;

/// Command line arguments.
#[derive(Parser, Debug)]
#[command(
    version,
    about = "Aries solver for flatzinc models.",
    long_about = None
)]
pub struct Args {
    /// Report all solutions
    #[arg(short, long)]
    pub all_solutions: bool,

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

/// Return command line args.
pub fn parse_args() -> Args {
    Args::parse()
}

/// Run the solver with the given args.
pub fn run(args: &Args) -> anyhow::Result<()> {
    let content = fs::read_to_string(&args.model)?;
    let model = parse_model(content.as_str())?;

    let print_all =
        args.all_solutions || model.is_optimize() && args.intermediate;

    let solver = Solver::new(model);

    if print_all {
        let print = |s| print!("{s}");
        make_output_flow(&solver, print)?;
    } else {
        let result = solver.solve()?;
        println!("{}", result.fzn());
    }
    Ok(())
}
