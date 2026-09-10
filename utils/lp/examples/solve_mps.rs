use aries_lp::{Error, MpsFile};
use std::io;

const USAGE: &str = "\
Read a problem in the MPS format and solve it.

USAGE:
    solve_mps --help
    solve_mps INPUT_FILE
    solve_mps INPUT_FILE --incremental

INPUT_FILE is a file in the M format. You can download some sample
problems from http://www.netlib.org/lp/data/. Use - for stdin.

Output is a single line containing the minimal objective value.

Set RUST_LOG environment variable (e.g. to info) to enable logging to stderr.
";

fn main() {
    env_logger::init();

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 && args.len() != 3 {
        print!("{}", USAGE);
        std::process::exit(1);
    } else if args[1] == "--help" {
        print!("{}", USAGE);
        return;
    } else if args.len() == 3 && args[2] != "--incremental" {
        print!("{}", USAGE);
        std::process::exit(1);
    }

    let is_incremental = args.len() == 3;

    let filename = &args[1];
    let direction = aries_lp::OptimizationDirection::Minimize;
    let file = if filename == "-" {
        MpsFile::parse(std::io::stdin().lock(), direction).unwrap()
    } else {
        let file = std::fs::File::open(filename).unwrap();
        let input = io::BufReader::new(file);
        MpsFile::parse(input, direction).unwrap()
    };

    let res_solve = if is_incremental {
        println!("Incremental");
        file.problem.solve_incremental()
    } else {
        println!("normal");
        file.problem.solve()
    };

    match res_solve {
        Ok(solution) => println!("status: OPTIMAL objective: {}", solution.objective() + file.obj_offset),
        Err(Error::InfeasibleTrivial) | Err(Error::InfeasibleWithCertificate(_)) => println!("status: INFEASIBLE"),
        Err(Error::Unbounded) => println!("status: UNBOUNDED"),
        Err(Error::Instable) => println!("status: INSTABLE"),
    }
}
