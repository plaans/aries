use anyhow::{Context, bail};
use aries::backtrack::Backtrack;
use aries::core::Lit;
use aries::model::extensions::SavedAssignment;
use aries::model::lang::expr::{Or, or};
use aries::solver::{Exit, UnsatCore};
use aries_explain::musmcs::marco::Marco;
use aries_explain::musmcs::marco::subsolvers::{MapSolverMode, SubsetSolverImpl};
use clap::Parser;
use itertools::Itertools;
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type Model = aries::model::Model<String>;
type Solver = aries::solver::Solver<String>;

#[derive(Parser, Debug)]
#[command()]
struct Opt {
    #[arg(long = "source")]
    source: Option<PathBuf>,
    file: PathBuf,
    // /// Timeout, in seconds
    // #[arg(long, short)]
    // timeout: Option<u64>,
}

enum Source {
    Dir(PathBuf),
    Zip(zip::ZipArchive<File>),
}

impl Source {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
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

    pub fn working_directory() -> anyhow::Result<Source> {
        Ok(Source::Dir(
            std::env::current_dir().context("Could not determine current directory")?,
        ))
    }

    pub fn read(&mut self, path: &Path) -> anyhow::Result<String> {
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

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let mut source = if let Some(f) = &opt.source {
        Source::new(f)?
    } else {
        Source::working_directory()?
    };

    let input = source.read(&opt.file)?;

    let cnf = varisat_dimacs::DimacsParser::parse(input.as_bytes())?;
    let (model, clauses) = load(cnf)?;

    find_muses_mcses(model, clauses)
}

struct SimpleSubsetSolverImpl {
    solver: Solver,
}
impl SimpleSubsetSolverImpl {
    pub fn new(model: Model) -> Self {
        Self {
            solver: Solver::new(model),
        }
    }
}
impl SubsetSolverImpl<String> for SimpleSubsetSolverImpl {
    fn get_model(&mut self) -> &mut Model {
        &mut self.solver.model
    }
    fn check_subset(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<Arc<SavedAssignment>, UnsatCore>, Exit> {
        let res = self
            .solver
            .solve_with_assumptions(subset.iter().copied().collect_vec())?;
        self.solver.reset();
        Ok(res)
    }
}

fn find_muses_mcses(model: Model, clauses: Vec<Or>) -> anyhow::Result<()> {
    let subset_solver_impl = Box::new(SimpleSubsetSolverImpl::new(model));

    let mut marco = Marco::with_soft_constraints_half_reif(clauses, subset_solver_impl, MapSolverMode::default());

    let _ = marco.run(
        Some(|mus: &BTreeSet<Lit>| println!("MUS found: {mus:?}")),
        Some(|mcs: &BTreeSet<Lit>| println!("MCS found: {mcs:?}")),
    );

    Ok(())
}

/// Load a CNF formula into a model and a set of constraints
pub fn load(cnf: varisat_formula::CnfFormula) -> anyhow::Result<(Model, Vec<Or>)> {
    let mut var_bindings = HashMap::new();
    let mut model = Model::new();

    let mut clauses: Vec<Or> = Vec::new();
    let mut clause_lits: Vec<Lit> = Vec::new();
    for clause in cnf.iter() {
        clause_lits.clear();
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
            clause_lits.push(lit);
        }
        // let c = model.half_reify(or(clause_lits.as_slice()));
        // clause_reifs.push(c);
        clauses.push(or(clause_lits.as_slice()));
    }

    Ok((model, clauses))
}
