pub(crate) mod ctags;
mod val;

use std::path::PathBuf;

use aries_planning_model::{
    Res,
    pddl::{self, input::Input},
};
use clap::*;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Val(Validate),
}

#[derive(Parser, Debug)]
pub struct Validate {
    plan: PathBuf,
    #[arg(short, long)]
    problem: Option<PathBuf>,
    #[arg(short, long)]
    domain: Option<PathBuf>,
}

fn main() -> Res<()> {
    let args = Args::parse();

    match &args.command {
        Commands::Val(command) => validate(command)?,
    }

    Ok(())
}

fn validate(command: &Validate) -> Res<()> {
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

    if val::validate(&model, &plan)? {
        println!("VALID")
    } else {
        println!("INVALID")
    }

    Ok(())
}
