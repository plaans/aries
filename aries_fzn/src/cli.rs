//! Command line interface.

use std::fs;
use std::path::PathBuf;

use anyhow::bail;
use aries::solver::search::SearchControl;
use aries::solver::search::beta::BetaBrancher;
use aries::solver::search::beta::restart::Never;
use aries::solver::search::beta::restart::RestartKind;
use aries::solver::search::beta::value_order::LowerHalf;
use aries::solver::search::beta::value_order::Max;
use aries::solver::search::beta::value_order::Min;
use aries::solver::search::beta::value_order::UpperHalf;
use aries::solver::search::beta::value_order::ValueOrderKind;
use aries::solver::search::beta::var_order::FirstFail;
use aries::solver::search::beta::var_order::Lexical;
use aries::solver::search::beta::var_order::VarOrderKind;
use clap::Parser;

use crate::aries::Solver;
use crate::fzn::Fzn;
use crate::fzn::parser::parse_model;
use crate::fzn::solution::make_output_flow;

fn parse_var_order(s: &str) -> anyhow::Result<VarOrderKind> {
    match s {
        "activity" => Ok(VarOrderKind::Activity(Default::default())),
        "first-fail" => Ok(VarOrderKind::FirstFail(FirstFail)),
        "lexical" => Ok(VarOrderKind::Lexical(Lexical)),
        _ => bail!("variable orders are activity, first-fail and lexical"),
    }
}

fn parse_value_order(s: &str) -> anyhow::Result<ValueOrderKind> {
    match s {
        "min" => Ok(ValueOrderKind::Min(Min)),
        "max" => Ok(ValueOrderKind::Max(Max)),
        "lower-half" => Ok(ValueOrderKind::LowerHalf(LowerHalf)),
        "upper-half" => Ok(ValueOrderKind::UpperHalf(UpperHalf)),
        "dynamic" => Ok(ValueOrderKind::Dynamic(Default::default())),
        _ => bail!(
            "value orders are min, max, lower-half, upper-half and dynamic"
        ),
    }
}

fn parse_restart(s: &str) -> anyhow::Result<RestartKind> {
    match s {
        "geometric" => Ok(RestartKind::Geometric(Default::default())),
        "never" => Ok(RestartKind::Never(Never)),
        _ => bail!("restart policies are geometric, never"),
    }
}

/// Command line arguments.
#[derive(Parser, Debug)]
#[command(
    version,
    about = "Aries solver for flatzinc models.",
    long_about = None
)]
pub struct Args {
    /// Report all solutions.
    #[arg(short, long)]
    pub all_solutions: bool,

    /// Report intermediate solutions.
    #[arg(short, long)]
    pub intermediate: bool,

    /// Variable order.
    #[arg(long, default_value = "lexical", value_name = "ORDER", value_parser = parse_var_order)]
    pub var_order: VarOrderKind,

    /// Value order.
    #[arg(long, default_value = "min", value_name = "ORDER", value_parser = parse_value_order)]
    pub value_order: ValueOrderKind,

    /// Restart policy.
    #[arg(long, default_value = "geometric", value_parser = parse_restart)]
    pub restart: RestartKind,

    /// Flatzinc model.
    #[arg(value_name = "FILE")]
    pub model: PathBuf,
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

    let mut solver = Solver::new(model);

    let brancher = BetaBrancher::new(
        args.var_order.clone(),
        args.value_order.clone(),
        args.restart.clone(),
    );
    solver.set_brancher(brancher.clone_to_box());

    if print_all {
        let print = |s| print!("{s}");
        make_output_flow(&solver, print)?;
    } else {
        let result = solver.solve()?;
        println!("{}", result.fzn());
    }
    Ok(())
}
