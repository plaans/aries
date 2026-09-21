mod encode;
mod groundings;
pub mod problem;

use aries_solver::prelude::*;

use encode::{encode_problem_ground, encode_problem_lifted};
use groundings::{SourceGrounding, TransitionGroundingId};
pub use problem::LpRelaxProblem;

use crate::analysis::Source;
use crate::analysis::grounding::ground_all_tasks;
use crate::analysis::transitions::supports::{Supports, SupportsSorted};
use crate::analysis::transitions::{TransitionId, Transitions};
use crate::{IntTerm, SchedEncoder, Task};

#[derive(Clone)]
pub(crate) struct LpRelaxEncoder {
    pub(crate) transitions: Transitions,
    pub(crate) supports: Supports,
    supports_sorted: Option<SupportsSorted>,

    sources_ground: groundings::SourcesGroundingsInfo,
    state_vars_ground: groundings::StateVarsGroundingsInfo,
    transitions_ground: groundings::TransitionsGroundingsInfo,
    terms_ground: groundings::TermsGroundingsInfo,
    supports_ground: groundings::SupportsGroundingsInfo,
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
            sources_ground: Default::default(),
            state_vars_ground: Default::default(),
            transitions_ground: Default::default(),
            terms_ground: Default::default(),
            supports_ground: Default::default(),
        }
    }

    pub fn post_ground_source(&mut self, source: Source, source_grounding: &SourceGrounding, ctx: &SchedEncoder) {
        // Will panic (in debug mode) if the source and the grounding have already been interned.
        let source_grounding_id = self.sources_ground.post_ground_source(source, source_grounding);

        // Intern each of the variable assignments in the source grounding,
        // marking each of them as appearing in it.
        {
            let source_terms = self.get_source_terms(source, ctx);

            for (i, &term) in source_terms.iter().enumerate() {
                let value = source_grounding[i];
                if !term.is_cst() {
                    self.terms_ground
                        .post_for_ground_source(term, value, source, source_grounding_id);
                }
            }
        }

        // For each transition of the source, intern the corresponding grounding,
        // marking each of them as being appearing in the source grounding.
        // Also mark each of the variable assignments as appearing in the corresponding transition groundings.
        {
            for &trans_id in self.transitions.of_source(source) {
                let trans_grounding = self.transitions.evaluate_terms(trans_id, source_grounding, ctx);

                let trans_grounding_id = TransitionGroundingId {
                    state_var_grounding_id: self.state_vars_ground.post_ground_state_var(
                        &self.transitions.get_state_var(trans_id, ctx).fluent,
                        trans_grounding.args_evaluated(),
                    ),
                    val_assignment: trans_grounding.val_evaluated(),
                    op_assignment: trans_grounding.op_evaluated(),
                };

                for (term, value) in self
                    .transitions
                    .iter_evaluated_non_constant_terms(trans_id, source_grounding, ctx)
                {
                    debug_assert!(!term.is_cst());

                    self.terms_ground
                        .post_for_ground_transition(term, value, trans_id, trans_grounding_id);
                }

                self.transitions_ground.post_ground_transition(
                    trans_id,
                    trans_grounding_id,
                    source,
                    source_grounding_id,
                );
            }
        }
    }

    pub fn sort(&mut self) {
        let time_all = std::time::Instant::now();
        let time = std::time::Instant::now();

        self.supports_sorted = Some(self.supports.sort());

        println!(
            "|-[LPRELAX]----- Sorted lifted supports in {}s",
            time.elapsed().as_secs_f64(),
        );
        let time = std::time::Instant::now();

        self.transitions_ground.sort_for_all();

        println!(
            "|-[LPRELAX]----- Sorted transitions groundings in {}s",
            time.elapsed().as_secs_f64(),
        );
        let time = std::time::Instant::now();

        self.supports_ground =
            groundings::SupportsGroundingsInfo::from(self.supports_sorted.as_ref().unwrap(), &self.transitions_ground);

        println!(
            "|-[LPRELAX]----- Built sorted support groundings in {}s",
            time.elapsed().as_secs_f64(),
        );
        let time = std::time::Instant::now();

        self.terms_ground.sort();

        println!(
            "|-[LPRELAX]----- Sorted terms groundings in {}s",
            time.elapsed().as_secs_f64(),
        );

        println!("|-[LPRELAX]--- Sorted all in {}s", time_all.elapsed().as_secs_f64(),);
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
    // #[allow(dead_code)]
    // pub fn iter_transitions(&self) -> impl Iterator<Item = (TransitionId, Transition)> {
    //     self.transitions.iter()
    // }

    pub fn encode(&mut self, ctx: &SchedEncoder, _doms: Option<&crate::Domains>) -> LpRelaxProblem {
        let time = std::time::Instant::now();

        let binding = ground_all_tasks(ctx);
        let sources_groundings = [(None, binding.empty_source_groundings().to_vec())].into_iter().chain(
            binding
                .all_task_groundings()
                .map(|(task, gs)| (Some(task), gs.to_vec())),
        );

        println!("|-[LPRELAX]--- Ran grounder in {}s", time.elapsed().as_secs_f64(),);

        let time = std::time::Instant::now();

        let mut n = 0;
        for (source, source_groundings) in sources_groundings {
            for source_grounding in source_groundings {
                self.post_ground_source(source, &source_grounding, ctx);
                n += 1;
            }
        }

        println!(
            "|-[LPRELAX]--- Interned {} groundings in {}s",
            n,
            time.elapsed().as_secs_f64(),
        );

        self.sort();

        let mut problem = LpRelaxProblem::default();

        let time = std::time::Instant::now();

        encode_problem_lifted(self, ctx, &mut problem);

        println!(
            "|-[LPRELAX]--- Collected lifted constraints in {}s",
            time.elapsed().as_secs_f64(),
        );
        let time = std::time::Instant::now();

        encode_problem_ground(self, ctx, &mut problem);

        println!(
            "|-[LPRELAX]--- Collected ground constraints in {}s",
            time.elapsed().as_secs_f64(),
        );

        problem
    }

    pub fn iter_sorted_all_only_assignments(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        self.terms_ground.iter_sorted_all_only_assignments()
    }
}

#[cfg(test)]
mod tests {}
