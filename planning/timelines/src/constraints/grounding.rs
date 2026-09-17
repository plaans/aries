use std::sync::Arc;

use aries_solver::{
    lang::{BoolExpr, Lit, ModelWrapper, constraints::Table},
    prelude::*,
};
use itertools::Itertools;

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
    fn enforce_if(&self, implicant: aries_solver::prelude::Lit, ctx: &mut SchedEncoder) {
        let groundings = crate::analysis::grounding::ground_all_tasks(ctx);

        println!();
        for (task, groundings) in groundings.all_task_groundings() {
            // println!("{task:?}, {}", groundings.len());

            let task_prez = ctx.sched.tasks[task].presence;
            let task_args = &ctx.sched.tasks[task].args;
            let task_args = task_args
                .iter()
                .map(|arg| VarCst::try_from(*arg).unwrap()) // TODO: proper conversion
                .collect_vec();

            if task_args.is_empty() {
                continue;
            }
            // dbg!(&groundings);

            let mut table = Table::new(task_args.len());

            for grounding in groundings {
                // println!("{:?}", grounding);
                table.push_line(grounding.get());
            }

            let has_grounding = in_table(task_args, Arc::new(table));
            let has_grounding = Scoped {
                constraint: has_grounding,
                scope: task_prez,
            };
            has_grounding.opt_enforce_if(implicant, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> aries_solver::prelude::Conjunction {
        Lit::TRUE.into()
    }
}

#[derive(Debug, Clone)]
struct Scoped<Constraint> {
    constraint: Constraint,
    scope: Lit,
}

impl<Ctx: Dom + ModelWrapper, Constraint: BoolExpr<Ctx>> BoolExpr<Ctx> for Scoped<Constraint> {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        self.constraint.enforce_if(implicant, ctx);
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        self.scope.into()
    }
}
