pub mod bindings;
mod encode;
mod ground;
pub mod problem;

use aries_solver::prelude::*;

use problem::LpRelaxProblem;

use crate::analysis::Source;
use crate::analysis::grounding::ground_all_tasks;
use crate::lprelax::transitions::{
    TransitionId, Transitions,
    supports::{Supports, SupportsSorted},
};
use crate::{IntTerm, SchedEncoder, Task};

/// Allows building the LP relaxation problem ([`LpRelaxProblem`]) of a scheduling problem, in two steps:
/// - [`Self::with_transitions_from`] collects the (lifted) transitions and the potential supports between them
///   (leaving out those that the given domains make temporally impossible);
/// - [`Self::encode`] grounds the sources, interning the groundings of their transitions and terms as well as the ground supports,
///   then encodes the lifted and ground rows.
#[derive(Clone)]
pub(crate) struct LpRelaxEncoder {
    pub(super) transitions: Transitions,
    /// The potential lifted supports between transitions.
    pub(super) supports: Supports,
    /// Sorted view on supports. Needed for efficient construction of rows / constraints relevant on ground supports.
    /// As such, should ideally be built once, after the ground sources have been interned.
    supports_sorted: Option<SupportsSorted>,

    /// The groundings of the sources, of their transitions and terms, and of the supports,
    /// interned as part of (or right before) encoding.
    ground: ground::GroundingsInfo,
}

impl LpRelaxEncoder {
    pub fn with_transitions_from(ctx: &SchedEncoder, dom: &impl Dom) -> Self {
        let transitions = Transitions::new_unambiguous(ctx);
        let supports = Supports::from(
            &transitions,
            ctx,
            dom,
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
    fn ground(&mut self, ctx: &SchedEncoder) {
        // Reusing those of `TasksUnifyWithGrounding` if it was enforced
        let groundings = match &ctx.grounding_tables {
            Some(tables) => {
                lprelax_log!("Reusing groundings used by the TasksUnifyWithGrounding constraint");
                tables.groundings.clone()
            }
            None => {
                let time = std::time::Instant::now();
                let res = ground_all_tasks(ctx);
                lprelax_log!("Got groundings in {}", time.elapsed().as_secs_f64(),);
                res
            }
        };

        let sources_groundings = [(None, groundings.empty_source_groundings().to_vec())]
            .into_iter()
            .chain(
                groundings
                    .all_task_groundings()
                    .map(|(task, gs)| (Some(task), gs.to_vec())),
            );

        let time = std::time::Instant::now();

        let mut n = 0;
        for (source, source_groundings) in sources_groundings {
            let source_terms = self.get_source_terms(source, ctx);
            // The literals of the lines of the source's table, if any (one per grounding, at the same index).
            // (Only when the tables are enforced unconditionally, see `encode_problem_ground`.)
            let table_line_lits = source.and_then(|task| {
                let tables = ctx.grounding_tables.as_ref()?;
                (tables.enforced_if == Lit::TRUE).then_some(tables.table_line_lits.get(task)?)
            });

            for (i, source_grounding) in source_groundings.into_iter().enumerate() {
                let table_line_lit = table_line_lits.map(|table_line_lits| table_line_lits[i]);
                self.ground.post_ground_source(
                    source,
                    source_terms,
                    &source_grounding,
                    table_line_lit,
                    &self.transitions,
                    ctx,
                );
                n += 1;
            }
        }

        lprelax_log!("Interned {} groundings in {}", n, time.elapsed().as_secs_f64(),);

        self.sort();
    }

    /// Grounds the sources (see [`Self::ground`]), then encodes the problem's lifted and ground rows.
    pub fn encode(&mut self, ctx: &SchedEncoder) -> LpRelaxProblem {
        self.ground(ctx);

        let mut problem = LpRelaxProblem::default();

        let time = std::time::Instant::now();

        encode::encode_problem_lifted(self, ctx, &mut problem);

        lprelax_log!("Collected lifted rows in {}", time.elapsed().as_secs_f64());

        let time = std::time::Instant::now();

        encode::encode_problem_ground(self, ctx, &mut problem);

        lprelax_log!("Collected ground rows in {}", time.elapsed().as_secs_f64());

        problem
    }

    pub fn iter_distinct_term_assignments(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        self.ground.iter_distinct_term_assignments()
    }
}
