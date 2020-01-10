#![allow(dead_code)]

#[derive(Debug)]
struct JobShop {
    pub num_jobs: usize,
    pub num_machines: usize,
    times: Vec<i32>,
    machines: Vec<usize>
}

impl JobShop {
    pub fn op_id(&self,  job: usize, op: usize) -> usize {
        job * self.num_machines + op
    }
    pub fn tvar(&self,  job: usize, op: usize) -> TVar {
        TVar(self.op_id(job, op) + 1)
    }
    pub fn duration(&self, job: usize, op: usize) -> i32 {
        self.times[job * self.num_machines + op]
    }
    pub fn machine(&self, job: usize, op: usize) -> usize {
        self.machines[job * self.num_machines + op]
    }
    pub fn op_with_machine(&self, job: usize, machine: usize) -> usize {
        for i in 0..self.num_machines {
            if self.machine(job, i) == machine {
                return i;
            }
        }
        panic!("This job is missing a machine")
    }
}

#[derive(Copy,Clone,Debug)]
struct TVar(usize);

impl Into<usize> for TVar {
    fn into(self) -> usize { self.0 }
}
const MAKESPAN: TVar = TVar(0);


use env_logger::Target;
use log::LevelFilter;
use std::fs;
use std::io::Write;
use structopt::StructOpt;
use aries::stn::STN;
use aries::core::all::BVar;
use aries::collection::MinVal;
use aries::collection::id_map::IdMap;

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

    println!("{:?}", pb);

    let mut stn = STN::new();
    stn.add_node(MAKESPAN, 0, 1000);
    for j in 0..pb.num_jobs {
        for i in 0..pb.num_machines {
            let tji = pb.tvar(j, i);
            stn.add_node(tji, 0, 1000);
            stn.record_constraint(tji, MAKESPAN, 0, true);
            if i > 0 {
                stn.record_constraint(pb.tvar(j, i-1), tji, - pb.duration(j, i-1), true);
            }
        }
    }
    let mut constraints = IdMap::new();
    let mut next_var = BVar::min_value();
    for m in 1..(pb.num_machines+1) {
        for j1 in 0..pb.num_jobs {
            for j2 in (j1 + 1)..pb.num_jobs {
                let i1 = pb.op_with_machine(j1, m);
                let i2 = pb.op_with_machine(j2, m);
                let v = next_var;
                next_var = next_var.next();

                let tji1 = pb.tvar(j1, i1);
                let tji2 = pb.tvar(j2, i2);
                let c1 = stn.record_constraint(tji1, tji2, - pb.duration(j1, i1), false);
                let c2 = stn.record_constraint(tji2, tji1, - pb.duration(j2, i2), false);
                constraints.insert(v, (c1, c2));

            }
        }
    }
    match aries::stn::domains(&stn) {
        Ok(doms) => {
            for j in 0..pb.num_jobs {
                for i in 0..pb.num_machines {
                    let tji = pb.tvar(j, i);
                    let start = doms[tji].min;
                    print!("{}\t ", start);

                    if i == pb.num_machines -1 {
                        println!("|{}", start + pb.duration(j, i));
                    }
                }
            }
            println!("Makespan = {}", doms[MAKESPAN].min);
        },
        Err(_) => println!("ERR")
    }
    println!("{:?}", aries::stn::domains(&stn))

}

fn parse(input: &String) -> JobShop {
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
            machines.push(t.parse().unwrap())
        }
    }

    JobShop { num_jobs, num_machines, times, machines }
}