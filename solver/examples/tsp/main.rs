use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use aries_bench_data::{IntermediateResult, Problem, SolveResult, SolveStatus, SolverMetric};
use aries_solver::prelude::*;

#[path = "../utils/mod.rs"]
mod utils;

mod parser_tsp;

use clap::Parser;

use crate::parser_tsp::parse_tsp;

/// This example provide a TSP solver following the .tsp format as described here: https://comopt.ifi.uni-heidelberg.de/software/TSPLIB95/
///
/// IMPORTANT: due to linear aspect of the problem, runtimes without the LP reasonner tends to be very important
/// therefore, the LP is enabled by default regardless the value of env var ARIES_LP_ENABLE
#[derive(Debug, Parser)]
#[command(name = "aries-tsp")]
struct Opt {
    /// File containing the instance to solve, a list of files can also be used.
    /// Use regex to specify directory: DIRECTORY/*.tsp
    ///
    /// If no arguments are given, the default folder will be used: /examples/tsp/instances
    files: Vec<PathBuf>,
    /// maximum runtime, in seconds.
    #[arg(long)]
    timeout: Option<u32>,
    /// If set, a summary of the run will be saved in the indicated directory.
    /// This option is intended to ease the collection of benchmark results with `aries-bench`
    #[arg(long)]
    report: Option<String>,
    /// If set, disable the lp reasonner inside aries
    #[arg(long)]
    no_lp: bool,
    /// If set, print some statistics and evolution of the cost
<<<<<<< HEAD
    #[arg(short, long)]
=======
    #[arg(long = "verbose")]
>>>>>>> 6f6f8aa3510d6069d28641597c522e0f72a27b68
    verbose: bool,
    /// If set, the solver will crash if it does not find the given optimum value.
    #[arg(long)]
    expected_value: Option<f64>,
}

/// Represents a square matrix with an empty diagonal
///
/// Used to store edges variables between nodes
struct DirectedSegmentMap<T> {
    n: usize,
    data: Vec<T>,
}

impl<T> DirectedSegmentMap<T> {
    pub fn new<F>(n: usize, mut init: F) -> Self
    where
        F: FnMut() -> T,
    {
        assert!(n > 1, "At least 2 nodes are necessary");
        let len = n * (n - 1);
        let mut data = Vec::with_capacity(len);

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    data.push(init());
                }
            }
        }

        Self { n, data }
    }

    #[inline]
    fn index(&self, i: usize, j: usize) -> usize {
        assert!(i < self.n && j < self.n, "Index out of bounds");
        assert!(i != j, "Line and column index must be different");

        // If j is greater than i, we shift it by -1 to fill the gap for i==j
        let j_adj = if j > i { j - 1 } else { j };
        i * (self.n - 1) + j_adj
    }

    pub fn get(&self, i: usize, j: usize) -> &T {
        &self.data[self.index(i, j)]
    }
}

#[derive(Debug)]
struct TspProblem {
    // Name of the instance
    name: String,
    /// Number of nodes
    n: usize,
    /// Set of weight between nodes
    weights: Vec<Vec<f64>>,
}

#[derive(Debug)]
struct TspSolution {
    /// Length of the tour
    cost: f64,
    /// Corresponds to the order in wich the cities has to be travelled
    tour_order: Vec<usize>,
}

/// Constant by which to multiply distances to get an integer without losing to much precision
const SCALE_FACTOR: f64 = 1000.0;

/// Solves a tsp problem and returns a TspSolution.
///
/// It is based on the [One Commodity Flow][ref-doc] representation that uses linear constraint exclusively
/// Constraints are named as in the paper
///
/// [ref-doc]: https://matmod.ch/lpl/PDF/tsp-2.pdf
fn solve_tsp(pb: &TspProblem, args: &Opt) -> Option<TspSolution> {
    // Used for report using benchmark
    let start_time = std::time::Instant::now();
    let mut solution_history: Vec<IntermediateResult> = Default::default();

    let mut model = Model::new();

    // Boolean variables expressing wether if an edge should be traversed or not
    let trav_edg = DirectedSegmentMap::new(pb.n, || model.new_variable(0, 1));

    // Integer variables to express the flow within the edges, check https://matmod.ch/lpl/PDF/tsp-2.pdf for more details
    let flow_edg = DirectedSegmentMap::new(pb.n, || model.new_variable(0, pb.n as IntCst - 1));

    let mut total_cost = LinSum::zero();

    for i in 0..pb.n {
        let mut sum_lin_trav_edg = LinSum::zero();
        let mut sum_col_trav_edg = LinSum::zero();

        let mut sum_lin_flow_edg = LinSum::zero();
        let mut sum_col_flow_edg = LinSum::zero();

        for j in 0..pb.n {
            if i == j {
                continue;
            }

            total_cost += *trav_edg.get(i, j) * (pb.weights[i][j] * SCALE_FACTOR).ceil() as IntCst;
            sum_lin_trav_edg += *trav_edg.get(i, j);
            sum_col_trav_edg += *trav_edg.get(j, i);

            // Force the flow of an edge to be 0 if it isn't traversed
            model.enforce(leq(*flow_edg.get(i, j), (pb.n as IntCst - 1) * *trav_edg.get(i, j))); // D3

            if i != 0 && j != 0 {
                // Force the flow to be less than n-2 for nodes that are not adjacent in the tour from the initial node
                model.enforce(leq(*flow_edg.get(i, j), (pb.n as IntCst - 2) * *trav_edg.get(i, j))); // G
            }

            sum_lin_flow_edg += *flow_edg.get(i, j);

            if j != 0 {
                sum_col_flow_edg += *flow_edg.get(j, i);
            }

            if j > i {
                // Force to have at most one edge active between 2 nodes
                model.enforce(leq(LinSum::zero() + *trav_edg.get(i, j) + *trav_edg.get(j, i), 1)); // S
            }
        }

        // Force the nodes to be visited exactly once
        model.enforce(eq(sum_lin_trav_edg, 1)); // A
        model.enforce(eq(sum_col_trav_edg, 1)); // B

        if i == 0 {
            // Force the flow to be equal to n-1 from the initial node
            model.enforce(eq(sum_lin_flow_edg, pb.n as IntCst - 1)); // D2
        } else {
            // Force the outgoing flow of a node to be one higher that its ingoing flow (except for the initial node)
            model.enforce(eq(sum_lin_flow_edg - sum_col_flow_edg, 1)); // D1
        }
    }

    let total_cost_var = model.new_variable(0, INT_CST_MAX);

    model.enforce(eq(total_cost, total_cost_var));

    if args.verbose {
        println!("Solving...");
    }

    let limit = if let Some(timeout) = args.timeout {
        SearchLimit::duration_secs(timeout)
    } else {
        SearchLimit::None
    };

    let mut status = SolveStatus::SolvedOpt;

    let mut best_cost: Option<i64> = None;

    // create the solver and solve to optimal (with 180s timeout)
    let mut solver = Solver::new(model);

    if args.no_lp {
        solver.reasoners.lp.deactivate();
    } else {
        solver.reasoners.lp.activate();
    }

    let solution_opt = match solver.minimize_with_callback(
        total_cost_var,
        |obj, _| {
            best_cost = Some((obj as f64 / SCALE_FACTOR) as i64);
            if args.verbose {
                println!("New solution with cost: {}", best_cost.unwrap());
            }
            solution_history.push(IntermediateResult {
                timestamp: start_time.elapsed(),
                objective: best_cost.unwrap(),
            });
        },
        limit,
    ) {
        Ok(Some((_, sol))) => {
            if args.verbose {
                println!("== Optimal solution found ==");
            }

            let mut cost = 0.0;

            let mut tour_order = Vec::<usize>::new();

            let mut curr_idx = 0;

            'while_loop: while tour_order.len() != pb.n {
                tour_order.push(curr_idx + 1); // We shift the index as TSP problems start numerotation with 1

                for next_idx in 0..pb.n {
                    if curr_idx == next_idx {
                        continue;
                    }

                    if sol.eval(*trav_edg.get(curr_idx, next_idx)).unwrap() == 1 {
                        cost += pb.weights[curr_idx][next_idx];
                        curr_idx = next_idx;
                        continue 'while_loop;
                    }
                }

                panic!("No following node found, the solution contains an error");
            }

            Some(TspSolution { cost, tour_order })
        }
        Ok(None) => {
            status = SolveStatus::SolvedUnsat;
            println!("No solution");
            None
        }
        Err(_) => {
            println!("Timeout");
            status = SolveStatus::Timeout;
            None
        }
    };

    if args.verbose {
        solver.print_stats();
    }

    // Allow us to use aries-bench
    if let Some(report_dir) = args.report.as_ref() {
        let problem = Problem {
            name: pb.name.clone(),
            timeout: args
                .timeout
                .map(|t| Duration::from_secs(t as u64))
                .unwrap_or(Duration::MAX),
            flags: Default::default(),
        };

        // If we have an optimal solution, we take the exact cost, otherwise we take the best cost so far
        let objective_value = if let Some(solution) = solution_opt.as_ref() {
            Some(solution.cost as i64)
        } else {
            best_cost
        };

        let result = SolveResult {
            problem,
            status,
            runtime: start_time.elapsed(),
            objective_value,
            metrics: Default::default(),
            objective_history: solution_history,
        }
        .with_metric(SolverMetric::NumConflicts, solver.stats.num_conflicts as f64)
        .with_metric(SolverMetric::NumDecisions, solver.stats.num_decisions as f64)
        .with_metric(SolverMetric::NumDomUpdates, solver.stats.num_dom_updates as f64);

        let _ = result.save_to_dir(report_dir); // TODO: handle this error correctly
    }

    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());

    solution_opt
}

const TOLERANCE: f64 = 1e-3;

fn solve_tsp_from_file<P>(path: P, args: &Opt) -> Option<TspSolution>
where
    P: AsRef<Path>,
{
    let problem_str = fs::read_to_string(path).expect("No such file");

    let pb = parse_tsp(&problem_str, args.verbose);

    // println!("Problem: {:?}", pb);

    let solution_opt = solve_tsp(&pb, args);

    if let Some(solution) = solution_opt.as_ref() {
        println!(
            "Optimal solution found with cost {}: {:?}",
            solution.cost, solution.tour_order
        );

        if let Some(optimal) = args.expected_value {
            assert!(
                (optimal - solution.cost).abs() < TOLERANCE,
                "Optimal cost differs from the expected"
            )
        }
    } else {
        println!("Timeout before reaching an optimal solution");
    }

    solution_opt
}

fn main() {
    let args = Opt::parse();

    for file in args.files.clone() {
        // println!("{:?}", file);
        solve_tsp_from_file(file, &args);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_tsp_example() {
        let args = Opt {
            files: Vec::new(),
            timeout: None,
            report: None,
            no_lp: false,
            expected_value: None,
            verbose: true,
        };

        {
            let problem = TspProblem {
                name: String::from("example 1"),
                n: 3,
                weights: vec![vec![0.0, 1.0, 1.0], vec![1.0, 0.0, 1.0], vec![1.0, 1.0, 0.0]],
            };

            let expected_value = 3.0;

            let expected_tour_order_1 = vec![1, 2, 3];
            let expected_tour_order_2 = vec![1, 3, 2];

            if let Some(solution) = solve_tsp(&problem, &args) {
                assert_eq!(solution.cost, expected_value, "Optimal cost differs from the expected");

                assert!(
                    solution.tour_order == expected_tour_order_1 || solution.tour_order == expected_tour_order_2,
                    "Unvalid tour order"
                )
            } else {
                panic!("A solution for {} should have been found", problem.name);
            }
        }

        {
            let problem = TspProblem {
                name: String::from("example 2"),
                n: 4,
                weights: vec![
                    vec![0.0, 1.0, 2.0, 1.0],
                    vec![1.0, 0.0, 1.0, 2.0],
                    vec![2.0, 1.0, 0.0, 1.0],
                    vec![1.0, 2.0, 1.0, 0.0],
                ],
            };

            let expected_value = 4.0;

            let expected_tour_order_1 = vec![1, 2, 3, 4];
            let expected_tour_order_2 = vec![1, 4, 3, 2];

            if let Some(solution) = solve_tsp(&problem, &args) {
                assert_eq!(solution.cost, expected_value, "Optimal cost differs from the expected");

                assert!(
                    solution.tour_order == expected_tour_order_1 || solution.tour_order == expected_tour_order_2,
                    "Unvalid tour order"
                )
            } else {
                panic!("A solution for {} should have been found", problem.name);
            }
        }

        {
            let problem = TspProblem {
                name: String::from("example 3"),
                n: 5,
                weights: vec![
                    vec![0.0, 2.5, 9.1, 4.0, 1.5],
                    vec![2.5, 0.0, 3.2, 8.0, 6.0],
                    vec![9.1, 3.2, 0.0, 2.1, 7.3],
                    vec![4.0, 8.0, 2.1, 0.0, 3.5],
                    vec![1.5, 6.0, 7.3, 3.5, 0.0],
                ],
            };

            let expected_value = 12.8;

            let expected_tour_order_1 = vec![1, 5, 4, 3, 2];
            let expected_tour_order_2 = vec![1, 2, 3, 4, 5];

            if let Some(solution) = solve_tsp(&problem, &args) {
                assert_eq!(solution.cost, expected_value, "Optimal cost differs from the expected");

                assert!(
                    solution.tour_order == expected_tour_order_1 || solution.tour_order == expected_tour_order_2,
                    "Invalid tour order"
                );
            } else {
                panic!("A solution for {} should have been found", problem.name);
            }
        }
    }
}
