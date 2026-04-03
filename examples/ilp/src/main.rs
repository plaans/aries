mod problem;

use anyhow::*;
use aries::model::extensions::Shaped;
use aries::prelude::*;
use aries::reasoners::lprelax::{new_default_lit_implier, new_default_lplit_implier};
use aries::solver::{Exit, SearchLimit};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::problem::IlpProblem;

type Model = aries::model::Model<String>;
type Solver = aries::solver::Solver<String>;

#[derive(Parser)]
#[command(version, about, name = "aries-ilp")]
struct Cli {
    file: PathBuf,

    /// Don't use LP relaxation
    #[arg(long, default_value_t = false)]
    no_lprelax: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let input_file = std::fs::read_to_string(&cli.file)?;
    let use_lprelax = !cli.no_lprelax;

    let problem = match &cli.file.extension().and_then(std::ffi::OsStr::to_str) {
        Some("mps") => todo!(), // IlpProblem::from_mps(&input_file)?,
        Some("lp") => IlpProblem::from_lp(&input_file)?,
        _ => return Err(anyhow::anyhow!("Input file needs to be .mps or .lp")),
    };
    //println!("{:#?}", problem);

    let model = problem.encode_model()?;

    let mut solver = make_solver(&problem, model, use_lprelax);
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

        for (col_name, val) in &sol {
            println!("{:?} = {}", col_name, val)
        }
    }

    Ok(())
}

fn make_solver(problem: &IlpProblem, model: Model, use_lp_relax: bool) -> Solver {
    let mut solver = Solver::new(model);

    if use_lp_relax {
        let mut var_name_to_col_map = HashMap::new();

        for (name, (lb, ub)) in &problem.vars {
            let col = solver
                .reasoners
                .lprelax
                .add_column(Some((*lb).into()), Some((*ub).into()));

            let var = solver.get_var(name).unwrap();
            var_name_to_col_map.insert(name.clone(), col);

            solver
                .reasoners
                .lprelax
                .register_lit_implier(var, new_default_lit_implier(var, col));
            solver
                .reasoners
                .lprelax
                .register_lplit_implier(col, new_default_lplit_implier(var, col));
        }
        for (row_coefs, lb, ub) in problem.constrs.values() {
            let row_coefs = row_coefs
                .iter()
                .map(|(name, coef)| (*var_name_to_col_map.get(name).unwrap(), (*coef).into()));
            solver
                .reasoners
                .lprelax
                .add_row(row_coefs, Some((*lb).into()), Some((*ub).into()));
        }

        if let Some((obj_name, obj_coefs)) = &problem.obj {
            let obj_coefs = obj_coefs
                .iter()
                .map(|(name, coef)| (*var_name_to_col_map.get(name).unwrap(), (*coef).into()));

            let obj_var = solver.get_var(obj_name).unwrap();
            let obj_col = solver.reasoners.lprelax.add_objective_column(
                obj_var,
                obj_coefs,
                match problem.sense {
                    lp_parser_rs::model::Sense::Minimize => aries::reasoners::lprelax::LpOptimSense::Minimise,
                    lp_parser_rs::model::Sense::Maximize => aries::reasoners::lprelax::LpOptimSense::Maximise,
                },
            );

            solver
                .reasoners
                .lprelax
                .register_lit_implier(obj_var, new_default_lit_implier(obj_var, obj_col));
            solver
                .reasoners
                .lprelax
                .register_lplit_implier(obj_col, new_default_lplit_implier(obj_var, obj_col));
        }
    }

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
