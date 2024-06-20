use crate::search::{Model, Var};
use aries::core::Lit;
use aries::model::lang::expr::{leq, or};
use aries::model::lang::{IAtom, IVar};
use itertools::Itertools;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ProblemKind {
    JobShop,
    OpenShop,
    FlexibleShop,
}

impl std::str::FromStr for ProblemKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jobshop" | "jsp" => Ok(ProblemKind::JobShop),
            "openshop" | "osp" => Ok(ProblemKind::OpenShop),
            "flexshop" | "flexible" | "fsp" | "fjs" => Ok(ProblemKind::FlexibleShop),
            _ => Err(format!("Unrecognized problem kind: '{s}'")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Op {
    pub job: u32,
    pub op_id: u32,
    pub alternatives: Vec<Alt>,
}

impl Op {
    pub fn min_duration(&self) -> i32 {
        self.alternatives.iter().map(|a| a.duration).min().unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct Alt {
    pub machine: u32,
    pub duration: i32,
}

#[derive(Clone, Debug)]
pub struct Problem {
    pub kind: ProblemKind,
    pub num_jobs: u32,
    pub num_machines: u32,
    pub operations: Vec<Op>,
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
                    alternatives: vec![Alt { machine, duration }],
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

    pub fn ops(&self) -> impl Iterator<Item = &Op> + '_ {
        self.operations.iter()
    }

    pub fn ops_by_job(&self, job: u32) -> impl Iterator<Item = &Op> + '_ {
        self.ops().filter(move |op| op.job == job)
    }

    pub fn machines(&self) -> impl Iterator<Item = u32> {
        0..self.num_machines
    }
    pub fn jobs(&self) -> impl Iterator<Item = u32> {
        0..self.num_jobs
    }

    /// Computes a lower bound on the makespan as the maximum of the operation durations in each
    /// job and on each machine.
    pub fn makespan_lower_bound(&self) -> i32 {
        let max_of_jobs: i32 = self
            .jobs()
            .map(|j| self.ops_by_job(j).map(|op| op.min_duration()).sum())
            .max()
            .unwrap();

        let mut max_by_machine = vec![0; self.num_machines as usize];
        for op in self.ops() {
            if op.alternatives.len() == 1 {
                let alt = &op.alternatives[0];
                max_by_machine[alt.machine as usize] += alt.duration;
            }
        }
        let max_of_machines: i32 = max_by_machine.iter().max().copied().unwrap();

        max_of_jobs.max(max_of_machines)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct OpAltId {
    pub job: u32,
    pub op: u32,
    pub alt: u32,
}

#[derive(Clone)]
pub struct OperationAlternative {
    pub id: OpAltId,
    pub machine: u32,
    pub duration: i32,
    pub start: IVar,
    pub presence: Lit,
}

impl OperationAlternative {
    pub fn start(&self) -> IAtom {
        self.start.into()
    }

    pub fn end(&self) -> IAtom {
        self.start + self.duration
    }
}

#[derive(Clone)]
pub struct Encoding {
    makespan: IVar,
    ops: Vec<OperationAlternative>,
}

impl Encoding {
    pub fn new(pb: &Problem, lower_bound: i32, upper_bound: i32, m: &mut Model) -> Self {
        let makespan = m.new_ivar(lower_bound, upper_bound, Var::Makespan);

        let mut oops = Vec::new();

        for op in pb.ops() {
            let job_id = op.job;
            let op_id = op.op_id;

            for (alt_id, alt) in op.alternatives.iter().enumerate() {
                let id = OpAltId {
                    job: job_id,
                    op: op_id,
                    alt: alt_id as u32,
                };
                let presence = if op.alternatives.len() == 1 {
                    Lit::TRUE
                } else {
                    m.new_presence_variable(Lit::TRUE, Var::Presence(id)).true_lit()
                };
                let start = m.new_optional_ivar(0, upper_bound, presence, Var::Start(id));
                oops.push(OperationAlternative {
                    id,
                    machine: alt.machine,
                    duration: alt.duration,
                    start,
                    presence,
                })
            }
        }

        Encoding { makespan, ops: oops }
    }

    pub fn operations(&self, job: u32) -> impl Iterator<Item = u32> + '_ {
        self.ops
            .iter()
            .filter_map(move |o| if o.id.job == job { Some(o.id.op) } else { None })
            .sorted()
            .unique()
    }

    pub fn alternatives(&self, job: u32, op: u32) -> impl Iterator<Item = &OperationAlternative> + '_ {
        self.ops.iter().filter(move |alt| alt.id.job == job && alt.id.op == op)
    }

    pub fn all_alternatives(&self) -> impl Iterator<Item = &OperationAlternative> + '_ {
        self.ops.iter()
    }

    pub fn alternatives_on_machine(&self, machine: u32) -> impl Iterator<Item = &OperationAlternative> + '_ {
        self.all_alternatives().filter(move |a| a.machine == machine)
    }
}

pub(crate) fn encode(pb: &Problem, lower_bound: u32, upper_bound: u32) -> (Model, Encoding) {
    let lower_bound = lower_bound as i32;
    let upper_bound = upper_bound as i32;
    let mut m = Model::new();
    let e = Encoding::new(pb, lower_bound, upper_bound, &mut m);

    // enforce makespan after last task
    for oa in e.all_alternatives() {
        m.enforce(leq(oa.start + oa.duration, e.makespan), [oa.presence]);
    }

    // make sure we have exactly one alternative per operation
    for j in pb.jobs() {
        for op in e.operations(j) {
            let alts = e.alternatives(j, op).map(|a| a.presence).collect_vec();
            assert!(!alts.is_empty());
            assert!(
                alts.len() > 1 || alts[0] == Lit::TRUE,
                "Not a flexible problem but presence is not a tautology"
            );
            // at least one must hold
            m.enforce(or(alts.as_slice()), []);
            // all alternatives are mutually exclusive
            for (i, l1) in alts.iter().copied().enumerate() {
                for &l2 in &alts[i + 1..] {
                    m.enforce(or([!l1, !l2]), []);
                }
            }
        }
    }

    // for each machine, impose that any two alternatives do not overlap
    for machine in 0..(pb.num_machines) {
        let alts = e.alternatives_on_machine(machine).collect_vec();
        for (i, alt1) in alts.iter().enumerate() {
            for alt2 in &alts[i + 1..] {
                // variable that is true if alt1 comes first and false otherwise.
                // in any case, setting a value to it enforces that the two tasks do not overlap
                let scope = m.get_conjunctive_scope(&[alt1.presence, alt2.presence]);
                let prec = m.new_optional_bvar(scope, Var::Prec(alt1.id, alt2.id));

                m.bind(leq(alt1.end(), alt2.start), prec.true_lit());
                m.bind(leq(alt2.end(), alt1.start), prec.false_lit());
            }
        }
    }
    match pb.kind {
        ProblemKind::JobShop | ProblemKind::FlexibleShop => {
            // enforce total order between tasks of the same job
            for j in pb.jobs() {
                let ops = e.operations(j).collect_vec();
                for i in 1..ops.len() {
                    let op1 = ops[i - 1];
                    let op2 = ops[i];
                    for alt1 in e.alternatives(j, op1) {
                        for alt2 in e.alternatives(j, op2) {
                            m.enforce(leq(alt1.end(), alt2.start), [alt1.presence, alt2.presence])
                        }
                    }
                }
            }
        }
        ProblemKind::OpenShop => {
            // enforce non-overlapping between tasks of the same job
            for j in pb.jobs() {
                let ops = e.operations(j).collect_vec();
                for (i, op1) in ops.iter().copied().enumerate() {
                    for &op2 in &ops[i + 1..] {
                        for alt1 in e.alternatives(j, op1) {
                            for alt2 in e.alternatives(j, op2) {
                                // variable that is true if alt1 comes first and false otherwise.
                                // in any case, setting a value to it enforces that the two tasks do not overlap
                                let scope = m.get_conjunctive_scope(&[alt1.presence, alt2.presence]);
                                let prec = m.new_optional_bvar(scope, Var::Prec(alt1.id, alt2.id));

                                m.bind(leq(alt1.end(), alt2.start), prec.true_lit());
                                m.bind(leq(alt2.end(), alt1.start), prec.false_lit());
                            }
                        }
                    }
                }
            }
        }
    }

    (m, e)
}
