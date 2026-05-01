mod problem;

use anyhow::*;
use aries::model::extensions::Shaped;
use aries::prelude::*;
use aries::solver::{Exit, SearchLimit};
use aries_lprelax::{LpRelax, default_lit_implications, default_lplit_implications};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

    /// Timeout (seconds)
    #[arg(long, short)]
    timeout: Option<u64>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let input_file = std::fs::read_to_string(&cli.file)?;
    let use_lprelax = !cli.no_lprelax;

    let problem = match &cli.file.extension().and_then(std::ffi::OsStr::to_str) {
        Some("mps") => IlpProblem::from_mps(&input_file)?,
        Some("lp") => IlpProblem::from_lp(&input_file)?,
        _ => return Err(anyhow::anyhow!("Input file needs to be .mps or .lp")),
    };
    // println!("{:#?}", problem);

    let model = problem.encode_model()?;

    let mut solver = make_solver(&problem, model, use_lprelax);

    let limit = if let Some(timeout) = cli.timeout {
        SearchLimit::Deadline(Instant::now() + Duration::from_secs(timeout))
    } else {
        SearchLimit::None
    };
    let res = solve(&problem, &mut solver, limit)?;

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
            println!("{:?} = {}", col_name, val);
        }
    } else {
        println!("No solution !");
    }

    Ok(())
}

fn make_solver(problem: &IlpProblem, model: Model, use_lp_relax: bool) -> Solver {
    let extra_reasoners: Vec<Box<dyn aries::reasoners::Theory>> = if use_lp_relax {
        let mut lprelax = LpRelax::default();

        let mut var_name_to_col_map = HashMap::new();

        for (name, (lb, ub)) in &problem.vars {
            let col = lprelax.add_column(Some((*lb).into()), Some((*ub).into()));

            let var = model.get_var(name).unwrap();
            var_name_to_col_map.insert(name.clone(), col);

            lprelax.set_lit_implications(var, default_lit_implications(var, col));
            lprelax.set_lplit_implications(col, default_lplit_implications(var, col));
        }
        for (row_coefs, lb, ub) in problem.constrs.values() {
            let row_coefs = row_coefs
                .iter()
                .map(|(name, coef)| (*var_name_to_col_map.get(name).unwrap(), (*coef).into()));
            lprelax.add_row(row_coefs, Some((*lb).into()), Some((*ub).into()));
        }

        if let Some((obj_name, obj_coefs)) = &problem.obj {
            let obj_coefs = obj_coefs
                .iter()
                .map(|(name, coef)| (*var_name_to_col_map.get(name).unwrap(), (*coef).into()));

            let obj_var = model.get_var(obj_name).unwrap();
            let obj_col = lprelax.add_objective_column(
                obj_var,
                obj_coefs,
                match problem.sense {
                    lp_parser_rs::model::Sense::Minimize => aries_lprelax::LpOptimSense::Minimise,
                    lp_parser_rs::model::Sense::Maximize => aries_lprelax::LpOptimSense::Maximise,
                },
            );

            lprelax.set_lit_implications(obj_var, default_lit_implications(obj_var, obj_col));
            lprelax.set_lplit_implications(obj_col, default_lplit_implications(obj_var, obj_col));
        }
        vec![Box::new(lprelax)]
    } else {
        vec![]
    };

    Solver::with_extra_reasoners(model, extra_reasoners)
}

fn solve(problem: &IlpProblem, solver: &mut Solver, limit: SearchLimit) -> Result<Option<(i32, Solution)>, Exit> {
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
