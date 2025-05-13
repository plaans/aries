//! Command line interface.

use std::fs;
use std::ops::Deref;
use std::path::PathBuf;

use aries::solver::search::beta_brancher;
use aries::solver::search::beta_brancher::BetaBrancher;
use aries::solver::search::beta_brancher::Polarity;
use aries::solver::search::SearchControl;
use clap::Parser;
use clap::ValueEnum;

use crate::aries::Solver;
use crate::fzn::parser::parse_model;
use crate::fzn::solution::make_output_flow;
use crate::fzn::Fzn;

/// Thin wrapper around VarOrder for clap.
#[derive(Clone, Default, Debug)]
pub struct VarOrder(beta_brancher::VarOrder);

impl ValueEnum for VarOrder {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self(beta_brancher::VarOrder::Lexical),
            Self(beta_brancher::VarOrder::FirstFail),
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self.0 {
            beta_brancher::VarOrder::Lexical => Some("lexical".into()),
            beta_brancher::VarOrder::FirstFail => Some("first-fail".into()),
        }
    }
}

impl Deref for VarOrder {
    type Target = beta_brancher::VarOrder;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Thin wrapper around ValueOrder for clap.
#[derive(Clone, Copy, Default, Debug)]
pub struct ValueOrder(beta_brancher::ValueOrder);

impl ValueEnum for ValueOrder {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self(beta_brancher::ValueOrder::Bound(Polarity::Negative)),
            Self(beta_brancher::ValueOrder::Bound(Polarity::Positive)),
            Self(beta_brancher::ValueOrder::Half(Polarity::Negative)),
            Self(beta_brancher::ValueOrder::Half(Polarity::Positive)),
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self.0 {
            beta_brancher::ValueOrder::Bound(p) => match p {
                Polarity::Negative => Some("min".into()),
                Polarity::Positive => Some("max".into()),
            },
            beta_brancher::ValueOrder::Half(p) => match p {
                Polarity::Negative => Some("upper-half".into()),
                Polarity::Positive => Some("lower-half".into()),
            },
        }
    }
}

impl Deref for ValueOrder {
    type Target = beta_brancher::ValueOrder;

    fn deref(&self) -> &Self::Target {
        &self.0
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
    #[arg(long, default_value_t, value_enum, value_name = "ORDER")]
    pub var_order: VarOrder,

    /// Value order.
    #[arg(long, default_value_t, value_enum, value_name = "ORDER")]
    pub value_order: ValueOrder,

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

    let brancher = BetaBrancher::new(*args.var_order, *args.value_order);
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
