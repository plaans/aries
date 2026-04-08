pub(crate) mod ctags;
pub(crate) mod optimize_plan;
mod repair;
mod generate;
mod validate;

use std::path::PathBuf;

use aries_plan_engine::plans::lifted_plan;
use clap::*;
use planx::{
    Message, Res,
    errors::*,
    pddl::{self, input::Input},
};
use timelines::IntCst;

use crate::repair::RepairOptions;

/// Aries Planning Engine (APE)
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Logging level to use: one of "error", "warn", "info", "debug", "trace"
    #[structopt(short, long, default_value = "info")]
    log_level: tracing::Level,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// PDDL parser (problem and associated domain).
    ///
    /// Will parse the PDDL problem and print the corresponding model to the standard output or provided (hopefully useful) error messages if the problem could not be parsed.
    Parse(Parse),
    /// PDDL parser (domain file only)
    ParseDomain(ParseDomain),
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
    /// (Finite) planning problem solving (plan generation):
    /// find a solution plan using, at most, a given finite number of action instances for each template (schema).
    SolveFinite(SolveFiniteProblem),
    /// Domain repair: proposing fixes of a domain based on a valid plan.
    DomRepair(DomRepair),
    FindDomain(FindDomain),
    FindProblem(FindProblem),
}

#[derive(Parser, Debug)]
pub struct Parse {
    /// Path to the problem file
    problem_file: PathBuf,
    /// Path to the PDDL domain file.
    /// If not specified, we will attempt to automatically infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct ParseDomain {
    /// Path to the PDDL domain file.
    domain_file: PathBuf,
}

/// Find the domain of a problem file, from naming conventions.
///
/// The file name is printed on the standard output to enable using a command like:
///
///     > my-planner `ape find-domain $PROBLEM_FILE` $PROBLEM_FILE
#[derive(Parser, Debug)]
pub struct FindDomain {
    /// Path to the problem file
    problem_file: PathBuf,
}
/// Find the problem of a plan file, from naming conventions.
///
/// The command works similarly to `find-domain` and prints only the file name on the stdout
/// to enable nesting in other command.
#[derive(Parser, Debug)]
pub struct FindProblem {
    /// Path to the plan file
    plan_file: PathBuf,
}

#[derive(Parser, Debug)]
pub struct Validate {
    /// Expanded to provide command line options to get the plan, problem and domain
    #[command(flatten)]
    plan_pb: PlanAndProblem,
    /// If set, will print verbose output
    #[arg(short, long)]
    verbose: bool,
    /// If set, the plan is expected to be invalid,
    /// The process will exit with error code 1 if the plan is valid.
    #[arg(short, long)]
    invalid: bool,
    /// If set, the process will exit with error code 1 if the plan's objective value is not the one indicated.
    ///
    /// This is primarily intended to enable testing (e.g. that the validator reproduces reference results).
    #[arg(short, long)]
    expected_objective: Option<IntCst>,
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
pub struct SolveFiniteProblem {
    /// Expanded to provide command line options to get the problem and domain
    #[command(flatten)]
    pb: Problem,
    #[command(flatten)]
    options: generate::Options,
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
    // set up logger
    let subscriber = tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::Uptime::from(std::time::Instant::now()))
        // .without_time() // if activated, no time will be printed on logs (useful for counting events with `counts`)
        // .with_thread_ids(true)
        .with_max_level(args.log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match &args.command {
        Commands::Parse(command) => parse(command)?,
        Commands::ParseDomain(command) => parse_domain(command)?,
        Commands::FindDomain(command) => find_domain(command)?,
        Commands::FindProblem(command) => find_problem(command)?,
        Commands::Validate(command) => validate_plan(command)?,
        Commands::OptimizePlan(command) => optimize_plan(command)?,
        Commands::SolveFinite(command) => solve_finite_problem(command)?,
        Commands::DomRepair(command) => repair(command)?,
    }

    Ok(())
}

fn parse(command: &Parse) -> Res<()> {
    let problem_file = &command.problem_file;
    if !problem_file.exists() {
        return Err(Message::error(format!(
            "Problem file {} does not exist",
            problem_file.display()
        )));
    }

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match command.domain.as_ref() {
        Some(name) => name.clone(),
        None => pddl::find_domain_of(&problem_file).title(
            "Unable to automatically find the domain file. Consider specifying the domain with the option -d/--domain",
        )?, // TODO: this erases the previous, possibly more informative, error message
    };
    let domain_file = Input::from_file(&domain_file)?;

    let problem_file = Input::from_file(&problem_file)?;
    let domain = pddl::parse_pddl_domain(domain_file)?;
    let problem = pddl::parse_pddl_problem(problem_file)?;

    let model = pddl::build_model(&domain, &problem)?;

    println!("{model}");
    Ok(())
}

fn parse_domain(command: &ParseDomain) -> Res<()> {
    let domain_file = &command.domain_file;
    if !domain_file.exists() {
        return Err(Message::error(format!(
            "Problem file {} does not exist",
            domain_file.display()
        )));
    }

    let domain_file = Input::from_file(domain_file)?;

    let domain = pddl::parse_pddl_domain(domain_file)?;
    let problem = pddl::Problem::empty(&domain.name, "?");

    let model = pddl::build_model(&domain, &problem)?;

    println!("{model}");
    Ok(())
}

fn find_domain(command: &FindDomain) -> Res<()> {
    match pddl::find_domain_of(&command.problem_file) {
        Ok(path) => {
            print!("{}", path.display());
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn find_problem(command: &FindProblem) -> Res<()> {
    match pddl::find_problem_of(&command.plan_file) {
        Ok(path) => {
            print!("{}", path.display());
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn validate_plan(command: &Validate) -> Res<()> {
    let (dom, pb, plan) = command.plan_pb.parse()?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    let plan = lifted_plan::parse_lifted_plan(&plan, &model)?;
    if command.verbose {
        println!("\n===== Model ====\n\n{model}\n");
        println!("\n===== Plan ====\n\n{plan}\n");
    }

    match validate::validate(&model, &plan, &command.options)? {
        validate::ValidationResult::Valid { objective_value } => {
            println!("VALID");
            if command.invalid {
                std::process::exit(1);
            }
            match (command.expected_objective, objective_value) {
                (Some(_exptected), None) => {
                    return Message::error("expected an objective value to be computed").failed();
                }
                (Some(exptected), Some(computed)) if exptected != computed => {
                    return Message::error(format!("expected an objective value of {exptected}")).failed();
                }
                (_, _) => {}
            }
        }
        validate::ValidationResult::Invalid => {
            println!("INVALID");
            if !command.invalid {
                std::process::exit(1);
            }
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
    println!("\n===== Plan ====\n\n{plan}\n");

    optimize_plan::optimize_plan(&model, &plan, &command.options)
}

fn solve_finite_problem(command: &SolveFiniteProblem) -> Res<()> {
    let (dom, pb) = command.pb.parse()?;

    // processed model (from planx)
    let model = pddl::build_model(&dom, &pb)?;
    println!("{model}");

    generate::solve_finite_planning_problem(&model, &command.options)
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
    /// If not specified, we will attempt to automatically infer it based on the plan file.
    #[arg(short, long)]
    problem: Option<PathBuf>,
    /// Path to the PDDL domain file.
    /// If not specified, we will attempt to automatically infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
}
impl PlanAndProblem {
    /// Parses the domain, problem and plan and returns them.
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

        println!("> Domain: {}", dom.display());
        println!("> Problem: {}", pb.display());
        println!("> Plan: {}", plan.display());

        // raw PDDL model
        let dom = pddl::parse_pddl_domain(Input::from_file(dom)?)?;
        let pb = pddl::parse_pddl_problem(Input::from_file(pb)?)?;
        let plan = pddl::parse_plan(Input::from_file(plan)?)?;
        Ok((dom, pb, plan))
    }
}

/// Structure that specifies a problem file and (optionnally) a domain file.
#[derive(::clap::Args, Debug)]
pub struct Problem {
    /// Path to the PDDL problem file.
    problem: PathBuf,
    /// Path to the PDDL domain file.
    /// If not specified, we will attempt to automatically infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
}
impl Problem {
    /// Parses the domain and problem and returns them.
    /// If the the domain is not specified, the method will attempt to infer
    /// it from naming conventions.
    pub fn parse(&self) -> Res<(pddl::Domain, pddl::Problem)> {
        let pb = &self.problem;
        if !self.problem.exists() {
            return Err(Message::error(format!("Problem file does not exist: {}", pb.display())));
        }
        let dom = if let Some(dom) = &self.domain {
            dom
        } else {
            &pddl::find_domain_of(pb)?
        };

        println!("> Domain: {}", dom.display());
        println!("> Problem: {}", pb.display());

        // raw PDDL model
        let dom = pddl::parse_pddl_domain(Input::from_file(dom)?)?;
        let pb = pddl::parse_pddl_problem(Input::from_file(pb)?)?;
        Ok((dom, pb))
    }
}
