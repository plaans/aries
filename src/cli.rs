use std::fs;
use std::path::PathBuf;

use clap::value_parser;
use clap::Parser;

use crate::aries::solver::Solver;
use crate::fzn::output::make_output;
use crate::fzn::parser::parse_model;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Report all solutions
    #[arg(short, long)]
    pub all_solutions: bool,

    /// Stop after after N solutions
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 1,
        value_parser = value_parser!(u32).range(1..),
    )]
    pub nb_solutions: u32,

    /// Report intermediate solutions
    #[arg(short, long)]
    pub intermediate: bool,

    /// Ignore search annotations
    #[arg(short, long)]
    pub free_search: bool,

    /// Print search statistics
    #[arg(short, long)]
    pub statistics: bool,

    /// Use verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Run with N parallel threads
    #[arg(
        short = 'p',
        long,
        value_name = "N",
        default_value_t = 1,
        value_parser = value_parser!(u32).range(1..),
    )]
    pub nb_threads: u32,

    /// Set random seed
    #[arg(short, long, value_name = "SEED")]
    pub random_seed: Option<u64>,

    /// Set time limit in milliseconds
    #[arg(short, long, value_name = "MS")]
    pub time: Option<u64>,

    /// Flatzinc model
    #[arg(value_name = "FILE")]
    pub model: PathBuf,
}

pub fn parse_args() -> Args {
    Args::parse()
}

pub fn run(args: &Args) {
    let content = fs::read_to_string(&args.model).unwrap();
    let model = parse_model(content).unwrap();
    let solver = Solver::new(model);
    let result = solver.solve().unwrap();
    let output = make_output(result);
    println!("{output}");
}
