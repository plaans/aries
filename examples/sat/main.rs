#![allow(clippy::map_entry)]

use anyhow::*;
use aries_model::bounds::Bound;
use aries_model::lang::BAtom;
use aries_model::Model;
use aries_solver::signals::{Signal, Synchro};
use aries_solver::solver::search::activity::{ActivityBrancher, BranchingParams};
use aries_solver::solver::{Exit, Solver};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
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

    let mut source = if let Some(f) = &opt.source {
        Source::new(f)?
    } else {
        Source::working_directory()?
    };

    let input = source.read(&opt.file)?;

    let cnf = varisat_dimacs::DimacsParser::parse(input.as_bytes())?;
    let (model, constraints) = load(cnf)?;

    solve_multi_threads(model, constraints, &opt)
    // solve_single_threads(model, constraints, &opt)
}

fn solve_single_threads(model: Model, constraints: Vec<BAtom>, opt: &Opt) -> Result<()> {
    let mut solver = Solver::new_unsync(model);
    solver.enforce_all(&constraints);
    if solver.solve().unwrap() {
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

struct WorkerResult {
    id: usize,
    output: Result<bool, Exit>,
    solver: Solver,
}

fn solve_multi_threads(model: Model, constraints: Vec<BAtom>, opt: &Opt) -> Result<()> {
    let (snd, rcv) = crossbeam_channel::unbounded();
    let in_handler_snd = snd.clone();
    ctrlc::set_handler(move || {
        in_handler_snd
            .send(Signal::Interrupt)
            .expect("Error sending the interruption signal")
    })
    .unwrap();
    let sync = Synchro { signals: rcv };
    let (result_snd, result_rcv) = crossbeam_channel::unbounded();

    let mut solver = Solver::new(model, Some(sync));
    solver.enforce_all(&constraints);
    let spawn = |id: usize, mut solver: Solver, result_snd: Sender<WorkerResult>| {
        thread::spawn(move || {
            let output = solver.solve();
            let answer = WorkerResult { id, output, solver };
            result_snd.send(answer).expect("Error while sending message");
        });
    };

    let search_params = [
        Default::default(),
        BranchingParams {
            prefer_min_value: !BranchingParams::default().prefer_min_value,
            ..Default::default()
        },
        BranchingParams {
            allowed_conflicts: 10,
            increase_ratio_for_allowed_conflicts: 1.02,
            ..Default::default()
        },
        BranchingParams {
            allowed_conflicts: 1000,
            increase_ratio_for_allowed_conflicts: 1.2,
            ..Default::default()
        },
    ];

    let num_threads = 4;
    for i in 1..num_threads {
        let mut solver = solver.clone();
        solver.set_brancher(ActivityBrancher::with_params(search_params[i - 1].clone()));
        // solver.set_seed(i as u64 + 1);
        let result_snd = result_snd.clone();
        spawn(i, solver, result_snd);
    }
    // we do not need to clone anything for the last one
    solver.set_brancher(ActivityBrancher::with_params(search_params[num_threads - 1].clone()));
    spawn(num_threads, solver, result_snd);

    for _ in 0..num_threads {
        let result = result_rcv.recv()?;
        snd.send(Signal::Interrupt)?;
        println!("========= Worker {} ========", result.id);
        match result.output {
            Ok(true) => {
                println!("SAT");
                if opt.expected_satisfiability == Some(false) {
                    eprintln!("Error: expected UNSAT but got SAT");
                    std::process::exit(1);
                }
            }
            Ok(false) => {
                println!("UNSAT");
                if opt.expected_satisfiability == Some(true) {
                    eprintln!("Error: expected SAT but got UNSAT");
                    std::process::exit(1);
                }
            }
            Err(Exit::Interrupted) => println!("Interrupted"),
        }
        println!("{}", result.solver.stats);
    }

    Ok(())
}

/// Load a CNF formula into a model and a set of constraints
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
