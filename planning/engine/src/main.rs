pub(crate) mod ctags;
mod repair;

use std::path::PathBuf;

use clap::*;
use planx::{
    Res,
    pddl::{self, input::Input},
};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    DomRepair(DomRepair),
}

#[derive(Parser, Debug)]
pub struct DomRepair {
    /// Path the valid plan, for which the domain may be flawed.
    plan: PathBuf,
    /// Path to the valid PDDL problem file.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    problem: Option<PathBuf>,
    /// Path to the PDDL domain file that is supposedly incorrect.
    /// If not specified, we will attempt to automaticall infer it based on the plan file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
}

fn main() -> Res<()> {
    let args = Args::parse();

    match &args.command {
        Commands::DomRepair(command) => validate(command)?,
    }

    Ok(())
}

fn validate(command: &DomRepair) -> Res<()> {
    let plan = &command.plan;
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

    let dom = pddl::parse_pddl_domain(Input::from_file(dom)?)?;
    let pb = pddl::parse_pddl_problem(Input::from_file(pb)?)?;
    let plan = pddl::parse_plan(Input::from_file(plan)?)?;

    let model = pddl::build_model(&dom, &pb)?;
    let plan = pddl::build_plan(&plan, &model)?;
    println!("{model}");
    println!("{plan:?}");

    if repair::domain_repair(&model, &plan)? {
        println!("VALID")
    } else {
        println!("INVALID")
    }

    Ok(())
}
