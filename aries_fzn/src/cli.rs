//! Command line interface.

use std::fs;
use std::ops::Deref;
use std::path::PathBuf;

use aries::solver::search::beta::value_order::LowerHalf;
use aries::solver::search::beta::value_order::Max;
use aries::solver::search::beta::value_order::Min;
use aries::solver::search::beta::value_order::UpperHalf;
use aries::solver::search::beta::value_order::ValueOrderKind;
use aries::solver::search::beta::var_order::FirstFail;
use aries::solver::search::beta::var_order::Lexical;
use aries::solver::search::beta::var_order::VarOrderKind;
use aries::solver::search::beta::BetaBrancher;
use aries::solver::search::SearchControl;
use clap::Parser;
use clap::ValueEnum;

use crate::aries::Solver;
use crate::fzn::parser::parse_model;
use crate::fzn::solution::make_output_flow;
use crate::fzn::Fzn;

/// Thin wrapper around VarOrderKind for clap.
#[derive(Clone, Default, Debug)]
pub struct VarOrder(VarOrderKind);

impl ValueEnum for VarOrder {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self(VarOrderKind::Lexical(Lexical)),
            Self(VarOrderKind::FirstFail(FirstFail)),
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self.0 {
            VarOrderKind::Lexical(_) => Some("lexical".into()),
            VarOrderKind::FirstFail(_) => Some("first-fail".into()),
        }
    }
}

impl Deref for VarOrder {
    type Target = VarOrderKind;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Thin wrapper around ValueOrderKind for clap.
#[derive(Clone, Default, Debug)]
pub struct ValueOrder(ValueOrderKind);

impl ValueEnum for ValueOrder {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self(ValueOrderKind::Min(Min)),
            Self(ValueOrderKind::Max(Max)),
            Self(ValueOrderKind::LowerHalf(LowerHalf)),
            Self(ValueOrderKind::UpperHalf(UpperHalf)),
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self.0 {
            ValueOrderKind::Min(_) => Some("min".into()),
            ValueOrderKind::Max(_) => Some("max".into()),
            ValueOrderKind::LowerHalf(_) => Some("lower-half".into()),
            ValueOrderKind::UpperHalf(_) => Some("upper-half".into()),
        }
    }
}

impl Deref for ValueOrder {
    type Target = ValueOrderKind;

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

    let brancher = BetaBrancher::new((*args.var_order).clone(), (*args.value_order).clone());
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
