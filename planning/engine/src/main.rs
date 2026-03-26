pub(crate) mod ctags;
pub(crate) mod optimize_plan;
mod repair;
mod validate;

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
    /// Plan validation.
    ///
    /// Specify a plan, and we will attempt to determine if it is valid and provide an appropriate exit code.
    /// The plan is implicitly expected to be valid, unless explictly marked as `--invalid`.
    ///
    /// Exit codes:
    ///  - error (1) if the domain/problem/plan fails to parse (even if the plan is expected invalid)
    ///  - success (0) if the plan is valid (implicitly expected valid)
    ///  - success (0) if the plan is invalid AND the '--invalid' option was passed
    ///  - error (1) if the validity status does not match the expectation
    #[clap(verbatim_doc_comment)]
    Validate(Validate),
    /// Plan optimization: specify an input plan, metrics and relaxation options and get an optmized plan.
    OptimizePlan(OptimizePlan),
    /// Domain repair: proposing fixes of a domain based on a valid plan.
    DomRepair(DomRepair),
}

#[derive(Parser, Debug)]
pub struct Validate {
    /// Expanded to provide command line options to get the plan, problem and domain
    #[command(flatten)]
    plan_pb: PlanAndProblem,
    /// If set, the plan is expected to be invalid,
    /// The process will exit with error code 1 if the plan is valid.
    #[arg(short, long)]
    invalid: bool,
    #[command(flatten)]
    options: validate::Options,
}

#[derive(Parser, Debug)]
pub struct OptimizePlan {
    /// Expanded to provide command line options to get the plan, problem and domain
    #[command(flatten)]
    plan_pb: PlanAndProblem,
    #[command(flatten)]
    options: optimize_plan::Options,
}

#[derive(Parser, Debug)]
pub struct DomRepair {
    /// Expanded to provide command line options to get the plan, problem and domain
    #[command(flatten)]
    plan_pb: PlanAndProblem,
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
        Commands::Validate(command) => validate_plan(command)?,
        Commands::OptimizePlan(command) => optimize_plan(command)?,
        Commands::DomRepair(command) => match repair(command) {
            Ok(()) => {}
            Err(e) => {
                // We report the error here and exit normally to ease the integration with external tooling
                // but this is typically not something we would like to keep in a released version
                println!("{e}");
                println!("REPORT {}   ERROR", command.plan_pb.plan.display());
            }
        },
    }

    Ok(())
}

fn validate_plan(command: &Validate) -> Res<()> {
    let (dom, pb, plan) = command.plan_pb.parse()?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    let plan = lifted_plan::parse_lifted_plan(&plan, &model)?;
    println!("{model}");
    println!("{plan:?}");

    let valid = validate::validate(&model, &plan, &command.options)?;
    if valid {
        println!("Plan is valid!");
        if command.invalid {
            std::process::exit(1);
        }
    } else {
        println!("INVALID plan!");
        if !command.invalid {
            std::process::exit(1);
        }
    }
    Ok(())
}

fn optimize_plan(command: &OptimizePlan) -> Res<()> {
    let (dom, pb, plan) = command.plan_pb.parse()?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    let plan = lifted_plan::parse_lifted_plan(&plan, &model)?;
    println!("{model}");
    println!("{plan:?}");

    optimize_plan::optimize_plan(&model, &plan, &command.options)?;
    todo!()
}

fn repair(command: &DomRepair) -> Res<()> {
    let (dom, pb, plan) = command.plan_pb.parse()?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    let plan = lifted_plan::parse_lifted_plan(&plan, &model)?;
    println!("{model}");
    //println!("{plan:?}");

    let report = repair::domain_repair(&model, &plan, &command.options)?;
    println!(
        "REPORT {:<90} {}    {:>4} (#actions)",
        &command.plan_pb.plan.display(),
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

/// Structure that specifies a plan file and (optionnally) a problem and domain files.
#[derive(::clap::Args, Debug)]
pub struct PlanAndProblem {
    /// Path to the plan.
    plan: PathBuf,
    /// Path to the PDDL problem file.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    problem: Option<PathBuf>,
    /// Path to the PDDL domain file.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
}
impl PlanAndProblem {
    /// Parse the domain, problem and plan and returns them.
    /// If the the problem or domains are not specified, the method will attempt to infer
    /// them from naming conventions.
    pub fn parse(&self) -> Res<(pddl::Domain, pddl::Problem, pddl::Plan)> {
        let plan = &self.plan;
        if !self.plan.exists() {
            return Err(Message::error(format!("Plan file does not exist: {}", plan.display())));
        }
        let pb = if let Some(pb) = &self.problem {
            pb
        } else {
            &pddl::find_problem_of(plan)?
        };
        let dom = if let Some(dom) = &self.domain {
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
        Ok((dom, pb, plan))
    }
}
