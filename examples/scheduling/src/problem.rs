use crate::search::{Model, Var};
use aries::core::{Lit, VarRef};
use aries::model::lang::expr::{alternative, eq, leq, or};
use aries::model::lang::linear::LinearSum;
use aries::model::lang::max::{EqMax, EqMin};
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

    pub fn operation(&self, job: u32, op_id: u32) -> &Op {
        self.ops().find(move |op| op.job == job && op.op_id == op_id).unwrap()
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

/// Represents an operation that must be executed and is associated to one or more alternative.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Operation {
    pub job: u32,
    pub op: u32,
    start: IAtom,
    end: IAtom,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct OperationId {
    pub job: u32,
    pub op: u32,
    /// If this represents an alternative, id of the alternative.
    /// A `None` value is used to idenitfy the top-level `Operation`
    pub alt: Option<u32>,
}

/// Represents one alternative to an operation
#[derive(Clone)]
pub struct OperationAlternative {
    pub id: OperationId,
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

/// Encoding of a scheduling problem, where each operation and alternative is associated with its variables in the CSP.
#[derive(Clone)]
pub struct Encoding {
    makespan: IVar,
    operations: Vec<Operation>,
    alternatives: Vec<OperationAlternative>,
}

impl Encoding {
    pub fn new(pb: &Problem, lower_bound: i32, upper_bound: i32, m: &mut Model) -> Self {
        let makespan = m.new_ivar(lower_bound, upper_bound, Var::Makespan);

        let mut operations = Vec::new();
        let mut alternatives = Vec::new();

        for op in pb.ops() {
            let job_id = op.job;
            let op_id = op.op_id;

            for (alt_id, alt) in op.alternatives.iter().enumerate() {
                let id = OperationId {
                    job: job_id,
                    op: op_id,
                    alt: Some(alt_id as u32),
                };
                let presence = if op.alternatives.len() == 1 {
                    Lit::TRUE
                } else {
                    m.new_presence_variable(Lit::TRUE, Var::Presence(id)).true_lit()
                };
                let start = m.new_optional_ivar(0, upper_bound, presence, Var::Start(id));
                alternatives.push(OperationAlternative {
                    id,
                    machine: alt.machine,
                    duration: alt.duration,
                    start,
                    presence,
                });
            }

            // build the top-level operation
            let operation = if op.alternatives.len() == 1 {
                // a single alternative, reused its variables of the operation
                let alt = alternatives.last().unwrap();
                Operation {
                    job: job_id,
                    op: op_id,
                    start: alt.start(),
                    end: alt.end(),
                }
            } else {
                // more that one alternative, create new variables for start/end
                let id = OperationId {
                    job: job_id,
                    op: op_id,
                    alt: None,
                };
                Operation {
                    job: job_id,
                    op: op_id,
                    start: m.new_optional_ivar(0, upper_bound, Lit::TRUE, Var::Start(id)).into(),
                    end: m.new_optional_ivar(0, upper_bound, Lit::TRUE, Var::Start(id)).into(),
                }
            };
            operations.push(operation);
        }
        Encoding {
            makespan,
            operations,
            alternatives,
        }
    }

    pub fn operations_ids(&self, job: u32) -> impl Iterator<Item = u32> + '_ {
        self.alternatives
            .iter()
            .filter_map(move |o| if o.id.job == job { Some(o.id.op) } else { None })
            .sorted()
            .unique()
    }

    pub fn all_operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.operations.iter()
    }

    pub fn operation(&self, job: u32, op: u32) -> &Operation {
        self.all_operations()
            .find(move |alt| alt.job == job && alt.op == op)
            .unwrap()
    }

    pub fn alternatives(&self, job: u32, op: u32) -> impl Iterator<Item = &OperationAlternative> + '_ {
        self.alternatives
            .iter()
            .filter(move |alt| alt.id.job == job && alt.id.op == op)
    }

    pub fn all_alternatives(&self) -> impl Iterator<Item = &OperationAlternative> + '_ {
        self.alternatives.iter()
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

    // enforce makespan after last alternative
    for oa in e.all_alternatives() {
        m.enforce(leq(oa.end(), e.makespan), [oa.presence]);
    }

    // enforce makespan after last operation and minimal duration of operation (based on alternatives)
    for o in e.all_operations() {
        m.enforce(leq(o.end, e.makespan), []);
        let min_duration = pb.operation(o.job, o.op).min_duration();
        m.enforce(leq(o.start + min_duration, o.end), []);
    }

    // if set to true, use an encoding with the alternative constraint
    let use_alternative_constraint = true;

    // make sure we have exactly one alternative per operation
    for j in pb.jobs() {
        for op in e.operations_ids(j) {
            let operation = e.operation(j, op);

            if use_alternative_constraint {
                let starts = e.alternatives(j, op).map(|alt| alt.start()).collect_vec();
                m.enforce(alternative(operation.start, starts), []);
                let ends = e.alternatives(j, op).map(|alt| alt.end()).collect_vec();
                m.enforce(alternative(operation.end, ends), []);
            } else {
                // enforce that, if an alternative is present, it matches the operation
                for alt in e.alternatives(j, op) {
                    m.enforce(eq(operation.start, alt.start()), [alt.presence]);
                    m.enforce(eq(operation.end, alt.end()), [alt.presence]);
                }

                // presence literals of all alternatives
                let alts = e.alternatives(j, op).map(|a| a.presence).collect_vec();
                assert!(!alts.is_empty());
                assert!(
                    alts.len() > 1 || alts[0] == Lit::TRUE,
                    "Not a flexible problem but presence is not a tautology"
                );
                // at least one alternative must be present
                m.enforce(or(alts.as_slice()), []);
                // all alternatives are mutually exclusive
                for (i, l1) in alts.iter().copied().enumerate() {
                    for &l2 in &alts[i + 1..] {
                        m.enforce(or([!l1, !l2]), []);
                    }
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

        // variable that is bound to the start of first task executing on the machine
        let start_first = m.new_ivar(0, upper_bound, Var::Intermediate);
        let mut starts = alts.iter().map(|a| a.start).collect_vec();
        starts.push(e.makespan); // add the makespan as a fallback in case there are no tasks on this machine
        m.enforce(EqMin::new(start_first, starts), []);

        // variable bound to the end of the latest task executing on the machine
        let end_last = m.new_ivar(0, upper_bound, Var::Intermediate);
        let mut ends = alts.iter().map(|a| a.end()).collect_vec();
        ends.push(start_first.into()); // add the start (=makespan) as fallback if not tasks are scheduled on this machine
        m.enforce(EqMax::new(end_last, ends), []);
        m.enforce(leq(end_last, e.makespan), []);

        // sum of the duration of all tasks executing on the machine
        let mut dur_sum = LinearSum::zero();
        for alt in &alts {
            // TODO: this is currently a workaound a missing API
            if alt.presence.variable() != VarRef::ZERO {
                let i_prez = IVar::new(alt.presence.variable());
                // assumes that i_prez is a 0-1 variable where 1 indicates presence
                dur_sum += i_prez * alt.duration;
            } else {
                assert_eq!(alt.presence, Lit::TRUE);
                dur_sum += alt.duration;
            }
        }

        m.enforce((dur_sum + start_first).leq(end_last), []);
        // m.enforce(dur_sum.leq(e.makespan), []); // weaker version does not require the intermediate variables
    }
    match pb.kind {
        ProblemKind::JobShop | ProblemKind::FlexibleShop => {
            // enforce total order between tasks of the same job
            for j in pb.jobs() {
                let ops = e.operations_ids(j).collect_vec();
                for i in 1..ops.len() {
                    let op1 = ops[i - 1];
                    let op2 = ops[i];

                    let o1 = e.operation(j, op1);
                    let o2 = e.operation(j, op2);
                    m.enforce(leq(o1.end, o2.start), [])
                }
            }
        }
        ProblemKind::OpenShop => {
            // enforce non-overlapping between tasks of the same job
            for j in pb.jobs() {
                let ops = e.operations_ids(j).collect_vec();
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
