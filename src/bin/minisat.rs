use env_logger::Target;
use log::LevelFilter;
use std::fs;
use std::io::Write;
use structopt::StructOpt;


use log::{debug};



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

    let clauses = aries::core::cnf::CNF::parse(&filecontent).clauses;

    let mut solver = aries::core::Solver::init(clauses);
    let vars = solver.variables();
    let sat = solver.solve(&aries::core::SearchParams::default());
    match sat {
        true => {

            debug!("==== Model found ====");
            let model = solver.model();
            for v in solver.variables() {
                debug!("{:?} <- {:?}", v, model.get(&v));
            }
            println!("SAT");
            if opt.expected_satifiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        false => {
            println!("UNSAT");

            if opt.expected_satifiability == Some(true) {
                eprintln!("Error: expected SAT but got UNSAT");
                std::process::exit(1);
            }
        }
    }
}
