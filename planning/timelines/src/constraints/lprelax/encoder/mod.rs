mod ground;
pub mod problem;

use aries_solver::prelude::*;

use crate::analysis::Source;
use crate::analysis::grounding::ground_all_tasks;
use crate::lprelax::transitions::{
    TransitionId, Transitions,
    supports::{Supports, SupportsSorted},
};
use crate::{IntTerm, SchedEncoder, Task};

#[derive(Clone)]
pub(crate) struct LpRelaxEncoder {
    pub(crate) transitions: Transitions,
    pub(crate) supports: Supports,
    supports_sorted: Option<SupportsSorted>,

    ground: ground::GroundingsInfo,
}

impl LpRelaxEncoder {
    pub fn with_transitions_from(ctx: &SchedEncoder) -> Self {
        let transitions = Transitions::new_unambiguous(ctx, super::ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS.get());
        let supports = Supports::from(
            &transitions,
            ctx,
            super::ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS.get(),
        );
        Self {
            transitions,
            supports,
            supports_sorted: None,
            ground: Default::default(),
        }
    }

    fn sort(&mut self) {
        let time = std::time::Instant::now();
        self.supports_sorted = Some(self.supports.sort());
        lprelax_log!("Sorted lifted supports in {}", time.elapsed().as_secs_f64());

        self.ground.sort(self.supports_sorted.as_ref().unwrap());
    }

    pub fn get_source<'a>(&self, source: Source, ctx: &'a SchedEncoder) -> Option<&'a Task> {
        source.map(|task_id| &ctx.sched.tasks[task_id])
    }
    pub fn get_source_prez(&self, source: Source, ctx: &SchedEncoder) -> Lit {
        self.get_source(source, ctx).map_or(Lit::TRUE, |task| task.presence)
    }
    pub fn get_source_terms<'a>(&self, source: Source, ctx: &'a SchedEncoder) -> &'a [IntTerm] {
        source
            .map(|task_id| &ctx.sched.tasks[task_id].args)
            .unwrap_or(&ctx.sched.global_args)
    }
    pub fn iter_sources(&self) -> impl Iterator<Item = (Source, &Vec<TransitionId>)> {
        self.transitions.iter_of_sources()
    }

    /// Grounds the sources, then interns their groundings, those of their transitions and terms,
    /// and the ground supports between transitions.
    pub fn ground(&mut self, ctx: &SchedEncoder) {
        let time = std::time::Instant::now();

        let binding = ground_all_tasks(ctx);
        let sources_groundings = [(None, binding.empty_source_groundings().to_vec())].into_iter().chain(
            binding
                .all_task_groundings()
                .map(|(task, gs)| (Some(task), gs.to_vec())),
        );

        lprelax_log!("Ran grounder in {}", time.elapsed().as_secs_f64(),);

        let time = std::time::Instant::now();

        let mut n = 0;
        for (source, source_groundings) in sources_groundings {
            let source_terms = self.get_source_terms(source, ctx);
            for source_grounding in source_groundings {
                self.ground
                    .post_ground_source(source, source_terms, &source_grounding, &self.transitions, ctx);
                n += 1;
            }
        }

        lprelax_log!("Interned {} groundings in {}", n, time.elapsed().as_secs_f64(),);

        self.sort();
    }

    pub fn iter_distinct_term_assignments(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        self.ground.iter_distinct_term_assignments()
    }
}
