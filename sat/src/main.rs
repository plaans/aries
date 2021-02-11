#![allow(clippy::map_entry)]

use anyhow::Result;
use aries_model::lang::{BAtom, Bound};
use aries_model::Model;
use aries_smt::solver::SMTSolver;
use std::collections::HashMap;
use std::fs::File;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "minisat")]
struct Opt {
    file: String,
    /// Sets the initial polarity of the variables to True/False to serve as the preferred value for variables.
    /// If not set, the solver will use an arbitrary value.
    #[structopt(long)]
    polarity: Option<bool>,
    #[structopt(long = "sat")]
    expected_satisfiability: Option<bool>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let f = File::open(opt.file)?;
    let cnf = varisat_dimacs::DimacsParser::parse(f)?;
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
