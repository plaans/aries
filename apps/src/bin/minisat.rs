use aries_sat::cnf::CNF;
use aries_sat::{SearchParams, SearchStatus};
use std::fs;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "minisat")]
struct Opt {
    file: String,
    #[structopt(long = "sat")]
    expected_satisfiability: Option<bool>,
}

fn main() {
    let opt = Opt::from_args();

    let file_content = fs::read_to_string(opt.file).expect("Cannot read file");

    let clauses = CNF::parse(&file_content).expect("Invalid file content: ").clauses;

    let mut solver = aries_sat::Solver::with_clauses(clauses, SearchParams::default());
    match solver.solve() {
        SearchStatus::Solution => {
            println!("SAT");
            if opt.expected_satisfiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        SearchStatus::Unsolvable => {
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
