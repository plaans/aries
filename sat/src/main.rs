#![allow(clippy::map_entry)]

use anyhow::*;
use aries_model::lang::{BAtom, Bound};
use aries_model::Model;
use aries_smt::solver::SMTSolver;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "minisat")]
struct Opt {
    #[structopt(long = "source")]
    source: Option<PathBuf>,
    file: PathBuf,
    /// Sets the initial polarity of the variables to True/False to serve as the preferred value for variables.
    /// If not set, the solver will use an arbitrary value.
    #[structopt(long)]
    polarity: Option<bool>,
    #[structopt(long = "sat")]
    expected_satisfiability: Option<bool>,
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

    let mut source = if let Some(f) = opt.source {
        Source::new(&f)?
    } else {
        Source::working_directory()?
    };

    let input = source.read(&opt.file)?;

    let cnf = varisat_dimacs::DimacsParser::parse(input.as_bytes())?;
    let (model, constraints) = load(cnf)?;

    let mut solver = SMTSolver::new(model);
    solver.enforce_all(&constraints);
    // solver.solve();
    // solver.model.discrete.print();
    //
    // let mut solver = aries_sat::solver::Solver::with_clauses(clauses, SearchParams::default());
    // match opt.polarity {
    //     Some(true) => solver.variables().for_each(|v| solver.set_polarity(v, true)),
    //     Some(false) => solver.variables().for_each(|v| solver.set_polarity(v, false)),
    //     None => (),
    // };
    if solver.solve() {
        println!("SAT");
        if opt.expected_satisfiability == Some(false) {
            eprintln!("Error: expected UNSAT but got SAT");
            std::process::exit(1);
        }
    } else {
        println!("UNSAT");
        if opt.expected_satisfiability == Some(true) {
            eprintln!("Error: expected SAT but got UNSAT");
            std::process::exit(1);
        }
    }
    println!("{}", solver.stats);
    Ok(())
}

pub fn load(cnf: varisat_formula::CnfFormula) -> Result<(Model, Vec<BAtom>)> {
    let mut var_bindings = HashMap::new();
    let mut model = Model::new();
    let mut clauses = Vec::new();

    let mut lits: Vec<BAtom> = Vec::new();
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
            let lit: Bound = if lit.is_positive() { var.into() } else { !var };
            lits.push(lit.into());
        }
        clauses.push(model.or(&lits));
    }

    Ok((model, clauses))
}
