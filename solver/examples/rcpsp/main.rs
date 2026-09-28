mod parser;
mod problem;

use aries_solver::prelude::*;
use aries_solver::solver::{Exit, SearchLimit};
use clap::Parser;
use problem::{Encoding, Problem};
use std::time::{Duration, Instant};

/// A simple solver the (multi-mode) RCPSP problem in PSPLIB format.
#[derive(Parser)]
#[command(name = "aries-rcpsp")]
pub struct Opt {
    /// File containing the instance to solve, in the PSPLIB multi-mode format.
    file: String,
    /// Maximum runtime, in seconds.
    #[arg(long, short)]
    timeout: Option<u32>,
    /// When set, the solver will fail with an exit code of 1 if the found solution does not have this makespan.
    #[arg(long = "expected-makespan")]
    expected_makespan: Option<IntCst>,
}

fn main() {
    let opt = Opt::parse();
    let content = std::fs::read_to_string(&opt.file).expect("Could not read file");
    let pb = parser::parse(&content);
    println!(
        "{} jobs, {} modes, {} renewable and {} nonrenewable resources",
        pb.jobs.len(),
        pb.jobs.iter().map(|j| j.modes.len()).sum::<usize>(),
        pb.renewable_capacity.len(),
        pb.nonrenewable_capacity.len()
    );
    println!(
        "Makespan bounds: [{}, {}]",
        pb.makespan_lower_bound(),
        pb.makespan_upper_bound()
    );

    let (model, encoding) = problem::encode(&pb);
    let makespan = encoding.makespan;
    let mut solver = Solver::new(model);

    let deadline = opt
        .timeout
        .map(|t| SearchLimit::Deadline(Instant::now() + Duration::from_secs(t as u64)))
        .unwrap_or(SearchLimit::None);

    let start_time = Instant::now();
    let result = solver.minimize_with_callback(makespan, |obj, _| println!("  solution with makespan {obj}"), deadline);
    solver.print_stats();

    match result {
        Ok(Some((obj, sol))) => {
            println!("> OPTIMAL (makespan: {obj})");
            assert_eq!(obj, sol.value_of(makespan).unwrap());
            print_schedule(&pb, &encoding, &sol);
            check_solution(&pb, &encoding, &sol, obj);
            if let Some(expected) = opt.expected_makespan
                && expected != obj
            {
                panic!("expected makespan {expected}, found {obj}");
            }
        }
        Ok(None) => println!("> UNSATISFIABLE"),
        Err(Exit::Interrupted) => println!("> TIMEOUT (after {}s)", start_time.elapsed().as_secs()),
    }
}

/// Prints the selected mode and start time of each job.
fn print_schedule(pb: &Problem, e: &Encoding, sol: &Solution) {
    for j in 0..pb.jobs.len() {
        let mv = e.modes_of(j).find(|mv| sol.entails(mv.presence)).unwrap();
        println!("job {j}: mode {} at time {}", mv.mode, sol.var_domain(mv.start).lb);
    }
}

/// Independently checks that the solution selected by the solver is a valid schedule.
fn check_solution(pb: &Problem, e: &Encoding, sol: &Solution, makespan: IntCst) {
    // selected mode and start/end time of each job
    let selected: Vec<(usize, IntCst, IntCst)> = (0..pb.jobs.len())
        .map(|j| {
            let mv = e.modes_of(j).find(|mv| sol.entails(mv.presence)).unwrap();
            let start = sol.var_domain(mv.start).lb;
            (mv.mode, start, start + mv.duration)
        })
        .collect();

    // exactly one mode per job
    for j in 0..pb.jobs.len() {
        assert_eq!(
            e.modes_of(j).filter(|mv| sol.entails(mv.presence)).count(),
            1,
            "job {j}: multiple modes selected"
        );
    }

    // precedences and makespan
    for (i, successors) in pb.successors.iter().enumerate() {
        for &j in successors {
            assert!(
                selected[i].2 <= selected[j].1,
                "job {j} starts before the end of its predecessor {i}"
            );
        }
    }
    for (i, (_, _, end)) in selected.iter().enumerate() {
        assert!(*end <= makespan, "job {i} ends after the makespan");
    }

    // nonrenewable resources
    for (r, &capacity) in pb.nonrenewable_capacity.iter().enumerate() {
        let total: IntCst = selected
            .iter()
            .enumerate()
            .map(|(j, (mode, _, _))| pb.job(j).modes[*mode].nonrenewable_demand[r])
            .sum();
        assert!(
            total <= capacity,
            "nonrenewable resource {r}: consumption {total} > {capacity}"
        );
    }

    // renewable resources, checked at the start time of each selected mode
    for (r, &capacity) in pb.renewable_capacity.iter().enumerate() {
        for j in 0..selected.len() {
            let (mode, start, _) = selected[j];
            let consumption: IntCst = selected
                .iter()
                .enumerate()
                .filter(|(j2, (_, s2, e2))| *s2 <= start && *e2 > start && *j2 != j)
                .map(|(j2, (mode2, _, _))| pb.job(j2).modes[*mode2].renewable_demand[r])
                .sum();
            let demand = pb.job(j).modes[mode].renewable_demand[r];
            assert!(
                demand + consumption <= capacity,
                "renewable resource {r} overloaded at time {start}"
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Solves the given instance and returns the optimal makespan.
    fn solve(file: &str) -> IntCst {
        let content = std::fs::read_to_string(file).unwrap();
        let pb = parser::parse(&content);
        let (model, encoding) = problem::encode(&pb);
        let mut solver = Solver::new(model);
        let (makespan, solution) = solver.minimize(encoding.makespan, SearchLimit::None).unwrap().unwrap();
        check_solution(&pb, &encoding, &solution, makespan);
        makespan
    }

    #[test]
    fn test_tiny() {
        assert_eq!(solve("examples/rcpsp/instances/tiny.mm"), 10);
    }

    #[test]
    fn test_j1014_1() {
        assert_eq!(solve("examples/rcpsp/instances/j1014_1.mm"), 16);
    }

    #[test]
    fn test_j1030_3() {
        assert_eq!(solve("examples/rcpsp/instances/j1030_3.mm"), 17);
    }

    #[test]
    fn test_j1047_5() {
        assert_eq!(solve("examples/rcpsp/instances/j1047_5.mm"), 16);
    }
}
