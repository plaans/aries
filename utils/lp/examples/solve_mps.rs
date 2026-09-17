use aries_lp::{Error, MpsFile};
use clap::Parser;
use std::{io, path::PathBuf};

/// Read a problem in the MPS format and solve it.
///
/// Set RUST_LOG environment variable (e.g. to info) to enable logging to stderr.
#[derive(Parser)]
struct Args {
    /// Path to a file in the MPS format. You can download some sample
    /// problems from http://www.netlib.org/lp/data/.
    file: PathBuf,
    /// If set, the solver will panic if it does not find the indicated optimal value.
    #[arg(long)]
    expected_value: Option<f64>,
    /// If set, the solver will build the LP incrementally (intended for testing purposes).
    #[arg(long)]
    incremental: bool,
}

fn main() {
    let args = Args::parse();

    let is_incremental = args.incremental;

    let direction = aries_lp::OptimizationDirection::Minimize;
    let file = std::fs::File::open(args.file).unwrap();
    let input = io::BufReader::new(file);
    let file = MpsFile::parse(input, direction).unwrap();

    let res_solve = if is_incremental {
        println!("Incremental");
        file.problem.solve_incremental()
    } else {
        println!("normal");
        file.problem.solve()
    };

    match res_solve {
        Ok(solution) => {
            let optimum = solution.objective() + file.obj_offset;
            println!("status: OPTIMAL objective: {}", solution.objective() + file.obj_offset);
            if let Some(expected) = args.expected_value {
                assert!((optimum - expected).abs() < 1e-3)
            }
        }
        Err(Error::InfeasibleTrivial) | Err(Error::InfeasibleWithCertificate(_)) => {
            println!("status: INFEASIBLE");
            if let Some(expected) = args.expected_value {
                assert!(expected.is_nan());
            }
        }
        Err(Error::Unbounded) => {
            println!("status: UNBOUNDED");
            if let Some(expected) = args.expected_value {
                assert!(expected.is_infinite());
            }
        }
        Err(Error::Unstable) => {
            println!("status: UNSTABLE");
            std::process::exit(1)
        }
    };
}
