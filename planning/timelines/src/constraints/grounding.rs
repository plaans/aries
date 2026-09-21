use std::sync::Arc;

use aries_solver::{
    lang::{BoolExpr, Lit, constraints::Table},
    prelude::*,
};

use crate::encoder::SchedEncoder;

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
        let groundings = crate::analysis::grounding::ground_all_tasks(ctx);

        // println!();
        for (task, groundings) in groundings.all_task_groundings() {
            // println!("{task:?}, {}", groundings.len());

            let task_prez = ctx.sched.tasks[task].presence;
            let task_args = &ctx.sched.tasks[task].args;

            if task_args.is_empty() {
                continue;
            }
            // dbg!(&groundings);

            let mut table = Table::new(task_args.len());

            for grounding in groundings {
                // println!("{:?}", grounding);
                table.push_line(grounding.get());
            }

            in_table(task_args.clone(), Arc::new(table))
                .scoped(task_prez)
                .opt_enforce_if(implicant, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> aries_solver::prelude::Conjunction {
        Lit::TRUE.into()
    }
}
