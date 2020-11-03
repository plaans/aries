use aries_sat::cnf::CNF;
use aries_sat::solver::{SearchParams, SearchResult};
use std::fs;
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

fn main() {
    let opt = Opt::from_args();

    let file_content = fs::read_to_string(opt.file).expect("Cannot read file");

    let clauses = CNF::parse(&file_content).expect("Invalid file content: ").clauses;

    let mut solver = aries_sat::solver::Solver::with_clauses(clauses, SearchParams::default());
    match opt.polarity {
        Some(true) => solver.variables().for_each(|v| solver.set_polarity(v, true)),
        Some(false) => solver.variables().for_each(|v| solver.set_polarity(v, false)),
        None => (),
    };
    match solver.solve() {
        SearchResult::Solved(_) => {
            println!("SAT");
            if opt.expected_satisfiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        SearchResult::Unsolvable => {
            println!("UNSAT");

            if opt.expected_satisfiability == Some(true) {
                eprintln!("Error: expected SAT but got UNSAT");
                std::process::exit(1);
            }
        }
        _ => unreachable!(),
    }
    println!("{}", solver.stats);
}
