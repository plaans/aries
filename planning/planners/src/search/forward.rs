//! A search controller that mimics forward search for HTN planning.

use crate::encoding::refinements_of;
use crate::Model;
use aries::backtrack::{Backtrack, DecLvl};
use aries::core::{Lit, VarRef};
use aries::model::extensions::{AssignmentExt, Shaped};
use aries::model::lang::IVar;
use aries::solver::search::{Decision, SearchControl};
use aries::solver::stats::Stats;
use aries_planning::chronicles::{ChronicleInstance, FiniteProblem, SubTask, VarLabel, VarType};
use std::convert::TryFrom;
use std::sync::Arc;

struct Task<'a> {
    /// Index of the chronicle instance this task appears in
    instance_id: usize,
    /// Index of the task in the chronicle
    task_id: usize,
    /// Literal that is true iff the task is present in the problem
    presence: Lit,
    /// The task itself (start, end, name, arguments)
    details: &'a SubTask,
}

fn all_tasks(pb: &FiniteProblem) -> impl Iterator<Item = Task> + '_ {
    pb.chronicles.iter().enumerate().flat_map(|(instance_id, ch)| {
        ch.chronicle
            .subtasks
            .iter()
            .enumerate()
            .map(move |(task_id, details)| Task {
                instance_id,
                task_id,
                presence: ch.chronicle.presence,
                details,
            })
    })
}

/// Among all tasks that are present and have no refinement yet, selects the one with the earliest possible start time.
fn earliest_pending_task<'a>(pb: &'a FiniteProblem, model: &Model) -> Option<Task<'a>> {
    let present_tasks = all_tasks(pb).filter(|t| model.state.entails(t.presence));
    // keep only those whose decomposition is pending (i.e. we have no present refinements of it
    let pending = present_tasks.filter(|t| {
        refinements_of(t.instance_id, t.task_id, pb)
            .iter()
            .all(|refinement| !model.entails(refinement.presence))
    });
    pending.min_by_key(|t| model.f_domain(t.details.start).num.lb)
}

/// Returns an iterator over all variables that appear in the atoms in input on which we would like to branch
fn branching_variables<'a>(ch: &'a ChronicleInstance, model: &'a Model) -> impl Iterator<Item = VarRef> + 'a {
    use VarType::*;
    // varref that controls the start time of the chronicle
    let start_ref: VarRef = ch.chronicle.start.num.var.into();
    ch.parameters
        .iter()
        .filter_map(|&a| {
            if let Some(x) = a.int_view() {
                IVar::try_from(x).ok().map(VarRef::from)
            } else {
                None
            }
        })
        .filter(move |&v| match model.get_label(v) {
            Some(VarLabel(_, TaskStart(_) | TaskEnd(_) | ChronicleEnd | Horizon)) => {
                // Ignore those, they will be constrained later by the other chronicle instantiations,
                // unless the variable is the same as the one in the start reference of the chronicle.
                // This can happen when a single variable is used to represent several things such as the start and the end of the chronicle
                v == start_ref
            }
            _ => true,
        })
        .filter(move |&v| {
            // only keep a variable if it is not already bound
            let (lb, ub) = model.state.bounds(v);
            lb < ub
        })
}

/// Selects the chronicle with the lowest possible start time among chronicles that are
/// present and have at least one parameter that is not set.
fn earliest_pending_chronicle<'a>(pb: &'a FiniteProblem, model: &Model) -> Option<&'a ChronicleInstance> {
    let presents = pb.chronicles.iter().filter(|ch| model.entails(ch.chronicle.presence));
    let pendings = presents.filter(|&ch| branching_variables(ch, model).next().is_some());
    let pendings: Vec<_> = pendings.collect();
    // println!("{pendings:?}");
    pendings
        .iter()
        .copied()
        .min_by_key(|ch| model.f_domain(ch.chronicle.start).num.lb)
}

/// Returns an arbitrary unbound variable in the parameters of this chronicle.
fn next_chronicle_decision(ch: &ChronicleInstance, model: &Model) -> Lit {
    let v = branching_variables(ch, model)
        .next()
        .expect("No decision left to take for this chronicle");
    let (lb, ub) = model.state.bounds(v);
    assert!(lb < ub);
    // println!("Chronicle {:?}", model.get_label(v));
    // print!("    ");
    Lit::leq(v, lb)
}

/// Given a pending task, returns a literal that activates an arbitrary refinement.
fn next_refinement_decision(chronicle_id: usize, task_id: usize, pb: &FiniteProblem, model: &Model) -> Lit {
    for refi in &refinements_of(chronicle_id, task_id, pb) {
        debug_assert!(!model.entails(refi.presence));
        if !model.entails(!refi.presence) {
            // print!("[m] ");
            return refi.presence;
        }
    }
    panic!("No possible refinement for task.")
}

/// Implements a forward search for HTN planning.
///
/// Among all:
///  - tasks that are present and not decomposed, and
///  - action chronicles that are present and not fully instantiated,
/// Selects the one with the earliest possible start time (as given by the lower bound of its start expression).
/// If it is a task, it will make one of its decomposing methods present.
/// If it is a chronicle, it will bind one of its parameters.
///
/// Note that the implementation is currently focused on simplicity and could be made much more efficient
/// with incremental datastructures.
#[derive(Clone)]
pub struct ForwardSearcher {
    problem: Arc<FiniteProblem>,
    saved: DecLvl,
}

impl ForwardSearcher {
    pub fn new(pb: Arc<FiniteProblem>) -> ForwardSearcher {
        ForwardSearcher {
            problem: pb,
            saved: DecLvl::ROOT,
        }
    }
}

impl SearchControl<VarLabel> for ForwardSearcher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        let xx = earliest_pending_chronicle(&self.problem, model);
        let yy = earliest_pending_task(&self.problem, model);
        let res = match (xx, yy) {
            (Some(ch), Some(tsk)) => {
                let ch_est = model.int_bounds(ch.chronicle.start).0;
                let tsk_est = model.int_bounds(tsk.details.start).0;
                if ch_est <= tsk_est {
                    Some(next_chronicle_decision(ch, model))
                } else {
                    Some(next_refinement_decision(
                        tsk.instance_id,
                        tsk.task_id,
                        &self.problem,
                        model,
                    ))
                }
            }
            (Some(ch), None) => Some(next_chronicle_decision(ch, model)),
            (None, Some(tsk)) => Some(next_refinement_decision(
                tsk.instance_id,
                tsk.task_id,
                &self.problem,
                model,
            )),
            (None, None) => None,
        };
        // Nothing left, inside chronicles, just instantiate all TaskStart as early as possible, starting with the earliest one
        // Note: this useful for scheduling problems where we have several TaskStart(_) within the base chronicle
        let res = res.or_else(|| {
            model
                .state
                .variables()
                .filter(|&v| model.get_label(v).is_some())
                .filter(|&v| model.state.present(v) == Some(true))
                .filter(|&v| matches!(model.get_label(v), Some(VarLabel(_, VarType::TaskStart(_)))))
                .filter_map(|v| {
                    let (lb, ub) = model.state.bounds(v);
                    if lb < ub {
                        Some((v, lb, ub))
                    } else {
                        None
                    }
                })
                .min_by_key(|(_v, lb, ub)| (*lb, *ub))
                .map(|(v, lb, _ub)| v.leq(lb))
        });
        // if there is no branching variable left, select the first unbound labeled variable
        let res = res.or_else(|| {
            model
                .state
                .variables()
                .filter(|&v| model.get_label(v).is_some())
                .filter(|&v| model.state.present(v) == Some(true))
                .filter_map(|v| {
                    let (lb, ub) = model.state.bounds(v);
                    if lb < ub {
                        Some(v.leq(lb))
                    } else {
                        None
                    }
                })
                .next()
        });

        res.map(|l| {
            // println!(" --> {:?}    \t {:?}", model.get_label(l.variable()), l);
            Decision::SetLiteral(l)
        })
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<VarLabel> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for ForwardSearcher {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}
