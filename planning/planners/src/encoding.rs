use aries::model::lang::FAtom;
use aries::{
    core::{IntCst, Lit},
    model::lang::Kind,
};
pub use aries_planning::chronicles::analysis::CondOrigin;
use aries_planning::chronicles::*;
use env_param::EnvParam;
use std::collections::{BTreeSet, HashSet};

/// Temporal origin
pub const ORIGIN: IntCst = 0;

/// The maximum duration of the plan.
pub static HORIZON: EnvParam<IntCst> = EnvParam::new("ARIES_PLANNING_HORIZON", "10000");

/// Identifier of a condition
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub struct CondID {
    /// Index of the instance in which the condition appears
    pub instance_id: ChronicleId,
    /// Index of the condition in the instance
    pub cond_id: CondOrigin,
}

impl CondID {
    pub fn new_explicit(instance_id: usize, cond_id: usize) -> Self {
        Self {
            instance_id,
            cond_id: CondOrigin::ExplicitCondition(cond_id),
        }
    }
    pub fn new_post_increase(instance_id: usize, eff_id: usize) -> Self {
        Self {
            instance_id,
            cond_id: CondOrigin::PostIncrease(eff_id),
        }
    }
}

/// Identifier of an effect
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub struct EffID {
    /// Index of the chronicle instance in whihc the effect appears
    pub instance_id: ChronicleId,
    /// Index of the effect in the effects of the instance
    pub eff_id: usize,
    /// Whether the effect is an assignment
    pub is_assign: bool,
}
impl EffID {
    pub fn new(instance_id: usize, eff_id: usize, is_assign: bool) -> Self {
        Self {
            instance_id,
            eff_id,
            is_assign,
        }
    }
}
pub type ChronicleId = usize;

/// Tag used to identify the purpose of some literals in the problem encoding.
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub enum Tag {
    Support(CondID, EffID),
    Decomposition(TaskId, ChronicleId),
}

/// Metadata associated to an encoding.
#[derive(Clone, Default)]
pub struct Encoding {
    pub(crate) tags: BTreeSet<(Tag, Lit)>,
}
impl Encoding {
    pub fn tag(&mut self, lit: Lit, tag: Tag) {
        self.tags.insert((tag, lit));
    }
}

/// Iterator over all effects in a finite problem.
///
/// Each effect is associated with
/// - the ID of the chronicle instance in which the effect appears
/// - a literal that is true iff the effect is present in the solution.
pub fn effects(pb: &FiniteProblem) -> impl Iterator<Item = (EffID, Lit, &Effect)> {
    pb.chronicles.iter().enumerate().flat_map(|(instance_id, ch)| {
        ch.chronicle.effects.iter().enumerate().map(move |(eff_id, eff)| {
            (
                EffID::new(instance_id, eff_id, is_assignment(eff)),
                ch.chronicle.presence,
                eff,
            )
        })
    })
}

/// Returns true if the effect is an assignment effect.
pub fn is_assignment(eff: &Effect) -> bool {
    matches!(eff.operation, EffectOp::Assign(_))
}

/// Iterator over all assignment effects in a finite problem.
pub fn assignments(pb: &FiniteProblem) -> impl Iterator<Item = (EffID, Lit, &Effect)> {
    effects(pb).filter(|(_, _, eff)| is_assignment(eff))
}

/// Returns true if the effect is an increase effect.
pub fn is_increase(eff: &Effect) -> bool {
    matches!(eff.operation, EffectOp::Increase(_))
}

/// Iterator over all increase effects in a finite problem.
pub fn increases(pb: &FiniteProblem) -> impl Iterator<Item = (EffID, Lit, &Effect)> {
    effects(pb).filter(|(_, _, eff)| is_increase(eff))
}

/// Iterates over all conditions in a finite problem.
///
/// Each condition is associated with
/// - the ID of the chronicle instance in which the condition appears
/// - a literal that is true iff the condition is present in the solution.
pub fn conditions(pb: &FiniteProblem) -> impl Iterator<Item = (CondID, Lit, &Condition)> {
    pb.chronicles.iter().enumerate().flat_map(|(instance_id, ch)| {
        ch.chronicle
            .conditions
            .iter()
            .enumerate()
            .map(move |(cond_id, cond)| (CondID::new_explicit(instance_id, cond_id), ch.chronicle.presence, cond))
    })
}

/// Returns true if the state variable is numeric.
pub fn is_numeric(sv: &StateVar) -> bool {
    matches!(sv.fluent.return_type().into(), Kind::Int) || matches!(sv.fluent.return_type().into(), Kind::Fixed(_))
}

pub struct TaskRef<'a> {
    pub presence: Lit,
    pub start: FAtom,
    pub end: FAtom,
    pub task: &'a Task,
}

pub(crate) fn get_task_ref(pb: &FiniteProblem, id: TaskId) -> TaskRef {
    let ch = &pb.chronicles[id.instance_id];
    let t = &ch.chronicle.subtasks[id.task_id];
    TaskRef {
        presence: ch.chronicle.presence,
        start: t.start,
        end: t.end,
        task: &t.task_name,
    }
}

/// Finds all possible refinements of a given task in the problem.
///
/// The task it the task with id `task_id` in the chronicle instance with it `chronicle_id`.
pub fn refinements_of(instance_id: usize, task_id: usize, pb: &FiniteProblem) -> Vec<TaskRef> {
    let mut supporters = Vec::new();
    let target_origin = TaskId { instance_id, task_id };
    for ch in pb.chronicles.iter() {
        match &ch.origin {
            ChronicleOrigin::Refinement { refined, .. } if refined.contains(&target_origin) => {
                let task = ch.chronicle.task.as_ref().unwrap();
                supporters.push(TaskRef {
                    presence: ch.chronicle.presence,
                    start: ch.chronicle.start,
                    end: ch.chronicle.end,
                    task,
                });
            }
            _ => {}
        }
    }
    supporters
}

pub fn refinements_of_task(task: &Task, pb: &FiniteProblem, spec: &Problem) -> HashSet<usize> {
    let mut candidates = HashSet::new();
    for (template_id, template) in spec.templates.iter().enumerate() {
        if let Some(ch_task) = &template.chronicle.task {
            if pb.model.unifiable_seq(task.as_slice(), ch_task.as_slice()) {
                candidates.insert(template_id);
            }
        }
    }
    candidates
}
