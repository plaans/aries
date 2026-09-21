use std::sync::Arc;

use aries_solver::{
    lang::{BoolExpr, Lit, constraints::Table},
    prelude::*,
};

use crate::encoder::SchedEncoder;

/// A redundant constraint that forces tasks to match one of their groundings
///
/// More precisely the constraint:
///
/// - grounds the scheduling problem to determine
///   all possible instantiation of all tasks.
/// - for each tasks posts a table constraint that restrict the instantiation of the tasks arguments
///   to match a grounding.
///
/// The constraint is redundant but may improve propagation as it can leverage the reachability analysis of the grounding process.
///
/// TODO: the grounding process may be very expensive and consume all available memory. The grounding is done when enforcing the constraint
/// and discarded afterwards. We should eventually provide a way to 1) limit the memory consumption of the grounder and 2) reusing existing groundings.
#[derive(Clone, Debug)]
pub struct TasksUnifyWithGrounding {}

impl TasksUnifyWithGrounding {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for TasksUnifyWithGrounding {
    fn default() -> Self {
        Self::new()
    }
}

impl BoolExpr<SchedEncoder> for TasksUnifyWithGrounding {
    fn enforce_if(&self, implicant: Lit, ctx: &mut SchedEncoder) {
        // ground the problem
        // NOTE: potentially very expensive (and memory intensive)
        let groundings = crate::analysis::grounding::ground_all_tasks(ctx);

        for (task, groundings) in groundings.all_task_groundings() {
            // for each task, we impose that its parameters must match one of its grounding
            //
            // For a task `move(truck, from, to)` the constraint could look something like this
            //
            // (truck, from, to) in-table [
            //   (truck1, loc1, loc2),
            //   (truck1, loc2, loc3),
            //   ...
            // ]

            // presence of the task
            let task_prez = ctx.sched.tasks[task].presence;
            // arguments of the task
            let task_args = &ctx.sched.tasks[task].args;

            if task_args.is_empty() {
                continue;
            }

            // create a table constraint that constraint that contains the tuples
            // of valid instantiations
            let mut table = Table::new(task_args.len());

            for grounding in groundings {
                table.push_line(grounding.get());
            }

            // post the constraint, which is forced to have the same scope as the task
            in_table(task_args.clone(), Arc::new(table))
                .scoped(task_prez)
                .opt_enforce_if(implicant, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> aries_solver::prelude::Conjunction {
        // the constraint is always valid (but will post several sub-constraints, scoped by each task)
        Lit::TRUE.into()
    }
}
