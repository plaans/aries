use aries_solver::lang::constraints::{Cumulative, Pulse};
use aries_solver::prelude::*;

/// One execution mode of a job: its duration and the amount of each resource it consumes.
#[derive(Clone, Debug)]
pub struct Mode {
    pub duration: IntCst,
    pub renewable_demand: Vec<IntCst>,
    pub nonrenewable_demand: Vec<IntCst>,
}

/// A job of the project. Must be executed in exactly one of its modes.
#[derive(Clone, Debug)]
pub struct Job {
    pub modes: Vec<Mode>,
}

/// A multi-mode resource-constrained project scheduling problem.
///
/// The jobs are numbered `0..n`, where job `0` and job `n-1` are the dummy source and sink jobs
/// (with a single mode of duration 0). `successors[i]` contains the jobs that must start after
/// the completion of job `i`.
#[derive(Clone, Debug)]
pub struct Problem {
    pub jobs: Vec<Job>,
    pub successors: Vec<Vec<usize>>,
    pub renewable_capacity: Vec<IntCst>,
    pub nonrenewable_capacity: Vec<IntCst>,
}

impl Problem {
    /// A lower bound on the makespan: the longest path in the precedence graph,
    /// using the shortest duration of each job.
    pub fn makespan_lower_bound(&self) -> IntCst {
        let n = self.jobs.len();
        let mut est = vec![0; n];
        for i in 0..n {
            let min_duration = self.jobs[i].modes.iter().map(|m| m.duration).min().unwrap();
            for &j in &self.successors[i] {
                est[j] = est[j].max(est[i] + min_duration);
            }
        }
        est.iter()
            .zip(&self.jobs)
            .map(|(&start, job)| start + job.modes.iter().map(|m| m.duration).min().unwrap())
            .max()
            .unwrap()
    }

    /// An upper bound on the makespan: executing all jobs in sequence is always feasible.
    pub fn makespan_upper_bound(&self) -> IntCst {
        self.jobs
            .iter()
            .map(|j| j.modes.iter().map(|m| m.duration).max().unwrap())
            .sum()
    }

    pub fn job(&self, job: usize) -> &Job {
        &self.jobs[job]
    }
}

/// A mode of a job, together with its variables in the model.
#[derive(Clone, Debug)]
pub struct ModeVar {
    pub job: usize,
    pub mode: usize,
    pub duration: IntCst,
    pub start: Var,
    pub presence: Lit,
}

impl ModeVar {
    pub fn end(&self) -> VarCst {
        self.start + self.duration
    }
}

/// Result of the encoding of a problem: the variables that were created.
#[derive(Clone)]
pub struct Encoding {
    pub makespan: Var,
    pub modes: Vec<ModeVar>,
}

impl Encoding {
    pub fn modes_of(&self, job: usize) -> impl Iterator<Item = &ModeVar> {
        self.modes.iter().filter(move |m| m.job == job)
    }
}

/// Encodes the problem as a CSP model: all reasoning is delegated to the solver's
/// propagators and clause learning.
pub fn encode(pb: &Problem) -> (Model, Encoding) {
    let lower_bound = pb.makespan_lower_bound();
    let upper_bound = pb.makespan_upper_bound();
    let mut m = Model::new();
    let makespan = m.new_ivar(lower_bound, upper_bound, "makespan");

    // --- variables: one optional start-time variable per (job, mode) pair
    let mut modes = Vec::new();
    for (j, job) in pb.jobs.iter().enumerate() {
        for (k, mode) in job.modes.iter().enumerate() {
            let label = format!("job{j}-mode{k}");
            let (presence, start) = if job.modes.len() == 1 {
                (Lit::TRUE, m.new_ivar(0, upper_bound, format!("{label}-start")))
            } else {
                let presence = m
                    .new_presence_variable(Lit::TRUE, format!("{label}-present"))
                    .true_lit();
                let start = m.new_optional_ivar(0, upper_bound, presence, format!("{label}-start"));
                (presence, start)
            };
            modes.push(ModeVar {
                job: j,
                mode: k,
                duration: mode.duration,
                start,
                presence,
            });
        }
    }
    let e = Encoding { makespan, modes };

    // --- exactly one mode must be selected for each job
    for job in 0..pb.jobs.len() {
        let lits: Vec<Lit> = e.modes_of(job).map(|m| m.presence).collect();
        if lits.len() > 1 {
            // at least one mode
            m.enforce(or(lits.as_slice()));
            // modes are mutually exclusive
            for i in 0..lits.len() {
                for &l2 in &lits[i + 1..] {
                    m.enforce(or([!lits[i], !l2]));
                }
            }
        }
    }

    // --- the makespan is after the end of every mode
    for mv in &e.modes {
        m.enforce_scoped(leq(mv.end(), e.makespan), [mv.presence]);
    }

    // --- precedence constraints: a job starts after the end of all its predecessors
    for (i, successors) in pb.successors.iter().enumerate() {
        for &j in successors {
            for mi in e.modes_of(i) {
                for mj in e.modes_of(j) {
                    m.enforce_scoped(leq(mi.end(), mj.start), [mi.presence, mj.presence]);
                }
            }
        }
    }

    // --- renewable resources: at any point in time, the total consumption must be below capacity
    for (r, &capacity) in pb.renewable_capacity.iter().enumerate() {
        let pulses: Vec<Pulse> = e
            .modes
            .iter()
            .filter(|mv| pb.job(mv.job).modes[mv.mode].renewable_demand[r] > 0)
            .map(|mv| Pulse {
                present: mv.presence,
                start: mv.start.into(),
                end: mv.end(),
                consumption: pb.job(mv.job).modes[mv.mode].renewable_demand[r],
            })
            .collect();
        m.enforce(Cumulative::new(capacity, pulses));
    }

    // --- nonrenewable resources: the total consumption over the whole project must be below capacity
    for (r, &capacity) in pb.nonrenewable_capacity.iter().enumerate() {
        let mut consumption = LinSum::cst(0);
        for mv in &e.modes {
            let demand = pb.job(mv.job).modes[mv.mode].nonrenewable_demand[r];
            if demand > 0 {
                consumption += bool2int(mv.presence, &mut m) * demand;
            }
        }
        m.enforce(consumption.leq(capacity));
    }

    (m, e)
}
