use crate::problem::{Job, Mode, Problem};
use aries_solver::core::u32_to_cst;

/// Returns true if the line contains only whitespace and digits.
fn is_int_row(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c.is_whitespace())
}

fn ints(line: &str) -> Vec<usize> {
    line.split_whitespace().map(|t| t.parse().unwrap()).collect()
}

/// Parses an instance of the multi-mode RCPSP in the PSPLIB format (`.mm` files).
///
/// The layout of those files is:
///  - a header, with notably the number of jobs (including the dummy source and sink jobs)
///    and the number of resources of each type (renewable, nonrenewable, doubly constrained)
///  - a PRECEDENCE RELATIONS section: for each job, its number of modes and its successors
///  - a REQUESTS/DURATIONS section: for each job and mode, the duration and the amount of
///    each resource requested
///  - a RESOURCEAVAILABILITIES section: the capacity of each resource
pub fn parse(input: &str) -> Problem {
    let lines: Vec<&str> = input.lines().map(str::trim).collect();

    let num_jobs = header_value(&lines, "jobs (incl.");
    let num_renewable = header_value(&lines, "- renewable");
    let num_nonrenewable = header_value(&lines, "- nonrenewable");
    let num_doubly = header_value(&lines, "- doubly constrained");
    let num_resources = num_renewable + num_nonrenewable + num_doubly;

    // --- PRECEDENCE RELATIONS section
    let start = section_start(&lines, "PRECEDENCE RELATIONS");
    let mut successors: Vec<Vec<usize>> = Vec::with_capacity(num_jobs);
    let mut num_modes: Vec<usize> = Vec::with_capacity(num_jobs);
    let mut i = start;
    while successors.len() < num_jobs {
        let line = lines[i];
        i += 1;
        if !is_int_row(line) {
            continue;
        }
        let nums = ints(line);
        assert_eq!(nums[0], successors.len() + 1, "unexpected job number: {line}");
        num_modes.push(nums[1]);
        let succs = nums[3..].iter().map(|&s| s - 1).collect();
        successors.push(succs);
    }

    // --- REQUESTS/DURATIONS section
    let start = section_start(&lines, "REQUESTS/DURATIONS");
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for line in lines.iter().skip(start) {
        if line.contains("RESOURCEAVAILABILITIES") {
            break;
        }
        if is_int_row(line) {
            let nums = ints(line);
            // the first column (job number) is only present for the first mode of each job
            let row = if nums.len() == num_resources + 3 {
                nums[1..].to_vec()
            } else {
                nums
            };
            assert_eq!(row.len(), num_resources + 2, "unexpected request row: {line}");
            rows.push(row);
        }
    }

    // distribute the mode rows to jobs, in order
    let mut jobs = Vec::with_capacity(num_jobs);
    let mut idx = 0;
    for modes in &num_modes {
        let mut job_modes = Vec::with_capacity(*modes);
        for _ in 0..*modes {
            let row = &rows[idx];
            idx += 1;
            // a row is: mode, duration, demands...
            let duration = u32_to_cst(row[1] as u32);
            let demand = &row[2..];
            // columns are ordered: renewable, nonrenewable, doubly constrained
            let mut renewable_demand = demand[..num_renewable].to_vec();
            let mut nonrenewable_demand = demand[num_renewable..num_renewable + num_nonrenewable].to_vec();
            // a doubly constrained resource is both renewable and nonrenewable
            for &d in &demand[num_renewable + num_nonrenewable..] {
                renewable_demand.push(d);
                nonrenewable_demand.push(d);
            }
            job_modes.push(Mode {
                duration,
                renewable_demand: renewable_demand.iter().map(|&d| u32_to_cst(d as u32)).collect(),
                nonrenewable_demand: nonrenewable_demand.iter().map(|&d| u32_to_cst(d as u32)).collect(),
            });
        }
        jobs.push(Job { modes: job_modes });
    }
    assert_eq!(idx, rows.len(), "unexpected number of mode rows");

    // --- RESOURCEAVAILABILITIES section
    let start = section_start(&lines, "RESOURCEAVAILABILITIES");
    let capacities = lines
        .iter()
        .skip(start)
        .find(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .expect("no resource availabilities found");
    let capacities: Vec<usize> = ints(capacities);
    assert_eq!(capacities.len(), num_resources);
    let renewable_capacity = capacities[..num_renewable]
        .iter()
        .map(|&c| u32_to_cst(c as u32))
        .collect();
    let mut nonrenewable_capacity: Vec<_> = capacities[num_renewable..num_renewable + num_nonrenewable]
        .iter()
        .map(|&c| u32_to_cst(c as u32))
        .collect();
    for &c in &capacities[num_renewable + num_nonrenewable..] {
        nonrenewable_capacity.push(u32_to_cst(c as u32));
    }

    Problem {
        jobs,
        successors,
        renewable_capacity,
        nonrenewable_capacity,
    }
}

/// Returns the integer value in a header line of the form `label : value`, e.g. `jobs (incl. supersource/sink ) : 12`.
fn header_value(lines: &[&str], prefix: &str) -> usize {
    let line = lines
        .iter()
        .find(|l| l.starts_with(prefix))
        .expect("missing header line: {prefix}");
    line.split(':')
        .nth(1)
        .expect("no value in header line")
        .split_whitespace()
        .next()
        .expect("no value in header line")
        .parse()
        .expect("non-numeric value in header line")
}

/// Returns the index of the first line after the line containing the given marker.
fn section_start(lines: &[&str], marker: &str) -> usize {
    lines
        .iter()
        .position(|l| l.contains(marker))
        .expect("missing section: {marker}")
        + 1
}
