#![allow(dead_code)]

#[derive(Debug)]
struct JobShop {
    pub num_jobs: usize,
    pub num_machines: usize,
    times: Vec<usize>,
    machines: Vec<usize>
}

impl JobShop {
    pub fn duration(&self, job: usize, op: usize) -> usize {
        self.times[job * self.num_machines + op]
    }

    pub fn machine(&self, job: usize, op: usize) -> usize {
        self.machines[job * self.num_machines + op]
    }
}

use env_logger::Target;
use log::LevelFilter;
use std::fs;
use std::io::Write;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "jobshop")]
struct Opt {
    file: String,
    #[structopt(long = "makespan")]
    expected_makespan: Option<bool>,
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

    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let pb = parse(&filecontent);

    println!("{:?}", pb)
}

fn parse(input: &String) -> JobShop {
    let mut lines = input.lines();
    lines.next(); // drop header "num_jobs num_machines"
    let x: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let num_jobs = x[0].parse().unwrap();
    let num_machines = x[1].parse().unwrap();

    lines.next(); // drop "Times line
    let mut times = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for t in lines.next().unwrap().split_whitespace() {
            times.push(t.parse().unwrap())
        }
    }
    lines.next(); // drop "Times line
    let mut machines = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for t in lines.next().unwrap().split_whitespace() {
            machines.push(t.parse().unwrap())
        }
    }

    JobShop { num_jobs, num_machines, times, machines }
}