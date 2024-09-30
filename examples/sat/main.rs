#![allow(clippy::map_entry)]

use anyhow::*;
use aries::core::Lit;
use aries::model::lang::expr::or;
use aries::solver::parallel::{ParSolver, SolverResult};
use aries::solver::search::conflicts::{ConflictBasedBrancher, Params};
use aries::solver::Solver;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use structopt::StructOpt;

type Model = aries::model::Model<String>;

#[derive(Debug, StructOpt)]
#[structopt(name = "minisat")]
struct Opt {
    #[structopt(long = "source")]
    source: Option<PathBuf>,
    file: PathBuf,
    #[structopt(long = "sat")]
    expected_satisfiability: Option<bool>,
    /// Timeout of the solver, in seconds
    #[structopt(long, short)]
    timeout: Option<u64>,
    #[structopt(long, short, default_value = "")]
    search: String,
}

enum Source {
    Dir(PathBuf),
    Zip(zip::ZipArchive<File>),
}

impl Source {
    pub fn new(path: &Path) -> Result<Self> {
        if path.is_dir() {
            Ok(Source::Dir(path.to_path_buf()))
        } else if let Some(ext) = path.extension() {
            if ext == "zip" {
                let f = std::fs::File::open(path)?;
                let z = zip::ZipArchive::new(f)?;
                Ok(Source::Zip(z))
            } else {
                bail!("Unsupported source: {}", path.display())
            }
        } else {
            bail!("Unsupported source: {}", path.display())
        }
    }

    pub fn working_directory() -> Result<Source> {
        Ok(Source::Dir(
            std::env::current_dir().context("Could not determine current directory")?,
        ))
    }

    pub fn read(&mut self, path: &Path) -> Result<String> {
        match self {
            Source::Dir(base_dir) => {
                let file = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    base_dir.join(path)
                };
                std::fs::read_to_string(file).context("Could not read file")
            }
            Source::Zip(archive) => {
                let path = path.to_str().context("invalid filename")?;
                let mut f = archive.by_name(path)?;
                let mut result = String::new();
                f.read_to_string(&mut result)?;
                Ok(result)
            }
        }
    }
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let deadline = opt.timeout.map(|timeout| Instant::now() + Duration::from_secs(timeout));

    let mut source = if let Some(f) = &opt.source {
        Source::new(f)?
    } else {
        Source::working_directory()?
    };

    let input = source.read(&opt.file)?;

    let cnf = varisat_dimacs::DimacsParser::parse(input.as_bytes())?;
    let model = load(cnf)?;

    solve_multi_threads(model, &opt, deadline)
}

fn solve_multi_threads(model: Model, opt: &Opt, deadline: Option<Instant>) -> Result<()> {
    let choices: Vec<_> = model.state.variables().map(|v| Lit::geq(v, 1)).collect();
    let solver = Box::new(Solver::new(model));

    let search_params: Vec<_> = opt.search.split(",").collect();
    let num_threads = search_params.len();

    let mut par_solver = ParSolver::new(solver, num_threads, |id, solver| {
        let choices = choices.clone();
        let mut params = Params::default();
        for opt in search_params[id].split(":") {
            let handled = params.configure(opt);
            if !handled {
                panic!("UNSUPPORTED OPTION: {opt}")
            }
        }
        solver.set_brancher(ConflictBasedBrancher::with(choices, params))
    });

    match par_solver.solve(deadline) {
        SolverResult::Sol(_sol) => {
            println!("> SATISFIED");
            if opt.expected_satisfiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        SolverResult::Unsat => {
            println!("> UNSATISFIABLE");
            if opt.expected_satisfiability == Some(true) {
                eprintln!("Error: expected SAT but got UNSAT");
                std::process::exit(1);
            }
        }
        SolverResult::Timeout(_) => {
            println!("> TIMEOUT");
            if opt.expected_satisfiability.is_some() {
                eprintln!("Error: could not conclude on SAT or UNSAT within the allocated time");
                std::process::exit(1);
            }
        }
    }
    par_solver.print_stats();

    Ok(())
}

/// Load a CNF formula into a model and a set of constraints
pub fn load(cnf: varisat_formula::CnfFormula) -> Result<Model> {
    let mut var_bindings = HashMap::new();
    let mut model = Model::new();

    let mut lits: Vec<Lit> = Vec::new();
    for clause in cnf.iter() {
        lits.clear();
        for &lit in clause {
            let var = lit.var();
            let var = if let Some(var) = var_bindings.get(&var) {
                *var
            } else {
                let model_var = model.new_bvar(var.to_dimacs().to_string());
                var_bindings.insert(var, model_var);
                model_var
            };
            let lit: Lit = if lit.is_positive() { var.into() } else { !var };
            lits.push(lit);
        }
        model.enforce(or(lits.as_slice()), []);
    }

    Ok(model)
}
