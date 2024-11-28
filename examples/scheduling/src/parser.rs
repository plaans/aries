use aries::core::u32_to_cst;

use crate::problem::*;

fn is_comment(line: &str) -> bool {
    line.chars().any(|c| c == '#')
}

fn ints(input_line: &str) -> impl Iterator<Item = usize> + '_ {
    input_line.split_whitespace().map(|n| n.parse().unwrap())
}

/// an iterator over non commented lines
fn lines(input: &str) -> impl Iterator<Item = &str> + '_ {
    input.lines().filter(|l| !is_comment(l))
}

pub(crate) fn openshop(input: &str) -> Problem {
    let mut lines = lines(input);
    lines.next();
    let mut x = ints(lines.next().unwrap());
    let num_jobs = x.next().unwrap();
    let num_machines = x.next().unwrap();

    let mut times = Vec::with_capacity(num_machines * num_jobs);
    let mut machines = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for (op_id, duration) in ints(lines.next().unwrap()).enumerate() {
            times.push(u32_to_cst(duration as u32));
            machines.push(op_id);
        }
    }
    Problem::new(ProblemKind::OpenShop, num_jobs, num_machines, times, machines)
}

pub(crate) fn jobshop(input: &str) -> Problem {
    println!("{input}");
    let mut lines = input.lines();
    lines.next(); // drop header "num_jobs num_machines"
    let x: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let num_jobs = x[0].parse().unwrap();
    let num_machines = x[1].parse().unwrap();

    lines.next(); // drop "Times" line
    let mut times = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for t in lines.next().unwrap().split_whitespace() {
            times.push(t.parse().unwrap())
        }
    }
    lines.next(); // drop "Machines" line
    let mut machines = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for t in lines.next().unwrap().split_whitespace() {
            machines.push(t.parse::<usize>().unwrap() - 1)
        }
    }

    Problem::new(ProblemKind::JobShop, num_jobs, num_machines, times, machines)
}

pub(crate) fn flexshop(input: &str) -> Problem {
    println!("{input}");
    let mut lines = input.lines();

    let x: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let num_jobs = x[0].parse().unwrap();
    let num_machines = x[1].parse().unwrap();

    fn next(it: &mut impl Iterator<Item = usize>) -> u32 {
        it.next().unwrap() as u32
    }

    let mut operations = Vec::with_capacity((num_jobs * num_jobs) as usize);

    for job in 0..num_jobs {
        let line = lines.next().unwrap();
        let ints = &mut ints(line);
        let num_ops = next(ints);
        for op_id in 0..num_ops {
            let num_alts = next(ints);
            let mut alternatives = Vec::with_capacity(num_alts as usize);
            for _ in 0..num_alts {
                let machine = next(ints) - 1;
                let duration = u32_to_cst(next(ints));
                alternatives.push(Alt { machine, duration })
            }
            operations.push(Op {
                job,
                op_id,
                alternatives,
            })
        }
    }
    Problem {
        kind: ProblemKind::FlexibleShop,
        num_jobs,
        num_machines,
        operations,
    }
}
