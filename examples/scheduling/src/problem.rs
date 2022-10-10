use crate::search::{Model, Var};
use aries_model::lang::expr::leq;
use aries_model::lang::IVar;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProblemKind {
    JobShop,
    OpenShop,
}

impl std::str::FromStr for ProblemKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jobshop" | "jsp" => Ok(ProblemKind::JobShop),
            "openshop" | "osp" => Ok(ProblemKind::OpenShop),
            _ => Err(format!("Unrecognized problem kind: '{s}'")),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Op {
    pub job: u32,
    pub op_id: u32,
    pub machine: u32,
    pub duration: i32,
}

#[derive(Clone, Debug)]
pub struct Problem {
    pub kind: ProblemKind,
    pub num_jobs: u32,
    pub num_machines: u32,
    operations: Vec<Op>,
}

impl Problem {
    pub fn new(
        kind: ProblemKind,
        num_jobs: usize,
        num_machines: usize,
        times: Vec<i32>,
        machines: Vec<usize>,
    ) -> Problem {
        let num_ops = num_jobs * num_machines;
        assert!(num_ops == times.len() && num_ops == machines.len());
        let mut ops = Vec::with_capacity(num_ops);
        let mut i = 0;
        for job in 0..num_jobs {
            for op_id in 0..num_machines {
                let duration = times[i];
                let machine = machines[i] as u32;
                assert!(machine < (num_machines as u32));
                ops.push(Op {
                    job: job as u32,
                    op_id: op_id as u32,
                    machine,
                    duration,
                });
                i += 1;
            }
        }
        Problem {
            kind,
            num_jobs: num_jobs as u32,
            num_machines: num_machines as u32,
            operations: ops,
        }
    }

    pub fn op(&self, job: u32, op: u32) -> Op {
        self.operations[(job * self.num_machines + op) as usize]
    }

    pub fn duration(&self, job: u32, op: u32) -> i32 {
        self.op(job, op).duration
    }
    pub fn machines(&self) -> impl Iterator<Item = u32> {
        0..self.num_machines
    }
    pub fn jobs(&self) -> impl Iterator<Item = u32> {
        0..self.num_jobs
    }

    pub fn machine(&self, job: u32, op: u32) -> u32 {
        self.op(job, op).machine
    }
    pub fn op_with_machine(&self, job: u32, machine: u32) -> u32 {
        for i in 0..self.num_machines {
            if self.machine(job, i) == machine {
                return i;
            }
        }
        panic!("This job is missing a machine")
    }

    pub(crate) fn operations(&self) -> &[Op] {
        &self.operations
    }

    /// Computes a lower bound on the makespan as the maximum of the operation durations in each
    /// job and on each machine.
    pub fn makespan_lower_bound(&self) -> i32 {
        let max_by_jobs: i32 = (0..self.num_jobs)
            .map(|job| (0..self.num_machines).map(|task| self.duration(job, task)).sum::<i32>())
            .max()
            .unwrap();

        let max_by_machine: i32 = self
            .machines()
            .map(|m| {
                (0..self.num_jobs)
                    .map(|job| self.duration(job, self.op_with_machine(job, m)))
                    .sum()
            })
            .max()
            .unwrap();

        max_by_jobs.max(max_by_machine)
    }
}

pub(crate) fn encode(pb: &Problem, lower_bound: u32, upper_bound: u32) -> Model {
    let start = |model: &Model, j: u32, t: u32| IVar::new(model.shape.get_variable(&Var::Start(j, t)).unwrap());
    let end = |model: &Model, j: u32, t: u32| start(model, j, t) + pb.duration(j, t);

    let lower_bound = lower_bound as i32;
    let upper_bound = upper_bound as i32;
    let mut m = Model::new();

    let makespan_variable = m.new_ivar(lower_bound, upper_bound, Var::Makespan);
    for j in 0..pb.num_jobs {
        for m1 in 0..pb.num_machines {
            let task_start = m.new_ivar(0, upper_bound, Var::Start(j, m1));
            m.enforce(leq(task_start + pb.duration(j, m1), makespan_variable));
        }
    }
    for machine in 0..(pb.num_machines) {
        for j1 in 0..pb.num_jobs {
            for j2 in (j1 + 1)..pb.num_jobs {
                let i1 = pb.op_with_machine(j1, machine);
                let i2 = pb.op_with_machine(j2, machine);
                // variable that is true if (j1, i1) comes first and false otherwise.
                // in any case, setting a value to it enforces that the two tasks do not overlap
                let prec = m.new_bvar(Var::Prec(j1, i1, j2, i2));
                m.bind(leq(end(&m, j1, i1), start(&m, j2, i2)), prec.true_lit());
                m.bind(leq(end(&m, j2, i2), start(&m, j1, i1)), prec.false_lit());
            }
        }
    }
    match pb.kind {
        ProblemKind::JobShop => {
            // enforce total order between tasks of the same job
            for j in pb.jobs() {
                for i in 1..pb.num_machines {
                    m.enforce(leq(end(&m, j, i - 1), start(&m, j, i)));
                }
            }
        }
        ProblemKind::OpenShop => {
            // enforce non-overlapping between tasks of the same job
            for j in 0..pb.num_jobs {
                for m1 in 0..pb.num_machines {
                    for m2 in (m1 + 1)..pb.num_machines {
                        let prec = m.new_bvar(Var::Prec(j, m1, j, m2));
                        m.bind(leq(end(&m, j, m1), start(&m, j, m2)), prec.true_lit());
                        m.bind(leq(end(&m, j, m2), start(&m, j, m1)), prec.false_lit());
                    }
                }
            }
        }
    }

    m
}
