mod problem;

use anyhow::*;
use aries::model::extensions::Shaped;
use aries::prelude::*;
use aries::solver::{Exit, SearchLimit};
use clap::Parser;
use std::path::PathBuf;

use crate::problem::IlpProblem;

type Model = aries::model::Model<String>;
type Solver = aries::solver::Solver<String>;

#[derive(Parser)]
#[command(version, about, name = "aries-ilp")]
struct Cli {
    #[arg(long)]
    file: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let input = std::fs::read_to_string(&cli.file)?;

    let problem = match &cli.file.extension().and_then(std::ffi::OsStr::to_str) {
        Some("mps") => IlpProblem::from_mps(&input)?,
        Some("lp") => IlpProblem::from_lp(&input)?,
        _ => return Err(anyhow::anyhow!("Input file needs to be .mps or .lp")),
    };

    //println!("{:#?}", problem);

    let model = problem.encode_model()?;

    let mut solver = make_solver(&problem, model);
    let res = solve(&problem, &mut solver)?;

    solver.print_stats();
    //solver.model.print_state();

    if let Some((obj, sol)) = res {
        println!("obj = {}", obj);

        let mut sol = sol
            .bound_variables()
            .filter_map(|(col_name, val)| solver.get_label(col_name).map(|col_name| (col_name, val)))
            .collect::<Vec<_>>();
        sol.sort();

        /*for (col_name, val) in &sol {
            println!("{:?} = {}",col_name, val)
        }*/
    }

    Ok(())
}

fn make_solver(_problem: &IlpProblem, model: Model) -> Solver {
    let solver = Solver::new(model);

    //solver.reasoners.lprelax.

    solver
}

fn solve(problem: &IlpProblem, solver: &mut Solver) -> Result<Option<(i32, Solution)>, Exit> {
    let limit = SearchLimit::None;
    //let limit = SearchLimit::Deadline(Instant::now() + Duration::from_secs(15));
    //let limit = SearchLimit::NumConflicts(10000);

    if let Some((obj_name, _)) = &problem.obj {
        match problem.sense {
            lp_parser_rs::model::Sense::Minimize => solver.minimize_with_callback(
                solver.get_int_var(obj_name).unwrap(),
                |o, _s| println!("new sol found: obj: {o}"),
                limit,
            ),
            lp_parser_rs::model::Sense::Maximize => solver.maximize_with_callback(
                solver.get_int_var(obj_name).unwrap(),
                |o, _s| println!("new sol found: obj: {o}"),
                limit,
            ),
        }
    } else {
        solver.solve(limit).map(|sol| sol.map(|sol| (0, sol)))
    }
}
