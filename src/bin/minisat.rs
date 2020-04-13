use env_logger::Target;
use log::{LevelFilter, debug};
use std::fs;
use std::io::Write;
use structopt::StructOpt;
use aries::core::cnf::CNF;
use aries::core::all::Lit;
use aries::core::{SearchParams, SearchStatus};

#[derive(Debug, StructOpt)]
#[structopt(name = "arsat")]
struct Opt {
    file: String,
    #[structopt(long = "sat")]
    expected_satifiability: Option<bool>,
    #[structopt(short = "v")]
    verbose: bool,
}

fn main() {
    let opt = Opt::from_args();
    env_logger::builder()
        .filter_level(if opt.verbose {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .target(Target::Stdout)
        .init();

    log::debug!("Options: {:?}", opt);

    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let clauses = parse(&filecontent).clauses;

    let mut solver = aries::core::Solver::init(clauses, SearchParams::default());
    match solver.solve() {
        SearchStatus::Solution => {

            debug!("==== Model found ====");
            let model = solver.model();
            for v in solver.variables() {
                debug!("{} <- {}", v, model.get(v).unwrap());
            }
            println!("SAT");
            if opt.expected_satifiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        SearchStatus::Unsolvable => {
            println!("UNSAT");

            if opt.expected_satifiability == Some(true) {
                eprintln!("Error: expected SAT but got UNSAT");
                std::process::exit(1);
            }
        }
        _ => unreachable!()
    }
    println!("{}", solver.stats);
}

fn parse(input: &str) -> CNF {
    let mut cnf = CNF::new();
    let mut lines_iter = input.lines().filter(|l| l.chars().next() != Some('c'));
    let header = lines_iter.next();
    assert!(header.and_then(|h| h.chars().next()) == Some('p'));
    for l in lines_iter {
        let lits = l
            .split_whitespace()
            .map(|lit| lit.parse::<i32>().unwrap())
            .take_while(|i| *i != 0)
            .map(|l| Lit::from_signed_int(l))
            .collect::<Vec<_>>();

        cnf.add_clause(&lits[..]);
    }
    cnf
}
