pub(crate) mod ctags;
mod optimize_plan;
mod repair;

use std::path::PathBuf;

use aries_plan_engine::plans::lifted_plan;
use clap::*;
use planx::{
    Message, Res,
    pddl::{self, input::Input},
};

use crate::repair::RepairOptions;

/// Aries Planning Engine (APE)
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    OptimizePlan(OptimizePlan),
    /// Domain repair: proposing fixes of a domain based on a valid plan.
    DomRepair(DomRepair),
}

#[derive(Parser, Debug)]
pub struct OptimizePlan {
    /// Path to the valid plan that will serve as the base for optiization
    plan: PathBuf,
    /// Path to the valid PDDL problem file.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    problem: Option<PathBuf>,
    /// Path to the PDDL domain file that is supposedly incorrect.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
    #[command(flatten)]
    options: optimize_plan::Options,
}

#[derive(Parser, Debug)]
pub struct DomRepair {
    /// Path to the valid plan, for which the domain may be flawed.
    plan: PathBuf,
    /// Path to the valid PDDL problem file.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    problem: Option<PathBuf>,
    /// Path to the PDDL domain file that is supposedly incorrect.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
    #[command(flatten)]
    options: RepairOptions,
    /// If provided, we will check that the number of repairs found is the one given here.
    /// If not, we will terminate the process with an error.
    /// (mainly intended for automated verifications and integration tests)
    #[arg(short, long)]
    expected_repairs: Option<usize>,
}

fn main() -> Res<()> {
    let args = Args::parse();

    match &args.command {
        Commands::DomRepair(command) => match repair(command) {
            Ok(()) => {}
            Err(e) => {
                // We report the error here and exit normally to ease the integration with external tooling
                // but this is typically not something we would like to keep in a released version
                println!("{e}");
                println!("REPORT {}   ERROR", command.plan.display());
            }
        },
        Commands::OptimizePlan(command) => optimize_plan(command)?,
    }

    Ok(())
}

fn optimize_plan(command: &OptimizePlan) -> Res<()> {
    let plan = &command.plan;
    if !plan.exists() {
        return Err(Message::error(format!("Plan file does not exist: {}", plan.display())));
    }
    let pb = if let Some(pb) = &command.problem {
        pb
    } else {
        &pddl::find_problem_of(plan)?
    };
    let dom = if let Some(dom) = &command.domain {
        dom
    } else {
        &pddl::find_domain_of(pb)?
    };
    println!(
        "# Starting domain repair:\n - Domain:  {}\n - Problem: {}\n - Plan:    {}\n",
        dom.display(),
        pb.display(),
        plan.display()
    );

    // raw PDDL model
    let dom = pddl::parse_pddl_domain(Input::from_file(dom)?)?;
    let pb = pddl::parse_pddl_problem(Input::from_file(pb)?)?;
    let plan = pddl::parse_plan(Input::from_file(plan)?)?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    let plan = lifted_plan::parse_lifted_plan(&plan, &model)?;
    println!("{model}");
    println!("{plan:?}");

    optimize_plan::optimize_plan(&model, &plan, &command.options)?;
    todo!()
}

fn repair(command: &DomRepair) -> Res<()> {
    let plan = &command.plan;
    if !plan.exists() {
        return Err(Message::error(format!("Plan file does not exist: {}", plan.display())));
    }
    let pb = if let Some(pb) = &command.problem {
        pb
    } else {
        &pddl::find_problem_of(plan)?
    };
    let dom = if let Some(dom) = &command.domain {
        dom
    } else {
        &pddl::find_domain_of(pb)?
    };
    println!(
        "# Starting domain repair:\n - Domain:  {}\n - Problem: {}\n - Plan:    {}\n",
        dom.display(),
        pb.display(),
        plan.display()
    );

    // raw PDDL model
    let dom = pddl::parse_pddl_domain(Input::from_file(dom)?)?;
    let pb = pddl::parse_pddl_problem(Input::from_file(pb)?)?;
    let plan = pddl::parse_plan(Input::from_file(plan)?)?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    let plan = lifted_plan::parse_lifted_plan(&plan, &model)?;
    println!("{model}");
    //println!("{plan:?}");

    let report = repair::domain_repair(&model, &plan, &command.options)?;
    println!(
        "REPORT {:<90} {}    {:>4} (#actions)",
        &command.plan.display(),
        report,
        plan.operations.len()
    );
    if let Some(expected) = command.expected_repairs {
        match report.status {
            repair::RepairStatus::ValidPlan => assert_eq!(expected, 0, "Expected a valid plan (no repairs)"),
            repair::RepairStatus::SmallestFound(num_repairs) => assert_eq!(
                num_repairs, expected,
                "Got {num_repairs} instead of the expected {expected}"
            ),
            repair::RepairStatus::Unrepairable => panic!(),
        }
    }

    Ok(())
}
