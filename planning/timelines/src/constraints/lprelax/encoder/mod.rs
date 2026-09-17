mod encode;
pub mod groundings;
pub mod problem;

use aries_solver::prelude::*;

use crate::analysis::Source;
use crate::analysis::grounding::{ParametersAssignment, ground_all_tasks};
use crate::constraints::lprelax::encoder::encode::{encode_problem_ground, encode_problem_lifted};
use crate::constraints::lprelax::encoder::groundings::{
    SourcesGroundingsInfo, StateVarsGroundingsInfo, SupportsGroundingsInfo, TermsGroundingsInfo,
    TransitionsGroundingsInfo,
};
use crate::constraints::lprelax::encoder::problem::LpRelaxProblem;
use crate::constraints::lprelax::transitions::ground::TransitionGroundingId;
use crate::constraints::lprelax::transitions::supports::Supports;
use crate::constraints::lprelax::transitions::{TransitionId, Transitions};
use crate::encoder::SchedEncoder;
use crate::{IntTerm, Task};

#[derive(Clone)]
pub(crate) struct LpRelaxEncoder {
    pub(crate) transitions: Transitions,
    pub(crate) supports: Supports,

    pub(crate) sources_ground: SourcesGroundingsInfo,
    pub(crate) state_vars_ground: StateVarsGroundingsInfo,
    pub(crate) transitions_ground: TransitionsGroundingsInfo,
    pub(crate) terms_ground: TermsGroundingsInfo,
    pub(crate) supports_ground: SupportsGroundingsInfo,
    // sources_grounder: todo!();
}

impl LpRelaxEncoder {
    pub fn with_transitions_from(ctx: &SchedEncoder) -> Self {
        let transitions = Transitions::new_unambiguous(ctx, super::ARIES_LPRELAX_RECOVER_MIES.get());
        let supports = Supports::from(&transitions, ctx, None);
        Self {
            transitions,
            supports,
            sources_ground: Default::default(),
            state_vars_ground: Default::default(),
            transitions_ground: Default::default(),
            terms_ground: Default::default(),
            supports_ground: Default::default(),
        }
    }

    pub fn post_ground_source(&mut self, source: Source, source_grounding: &ParametersAssignment, ctx: &SchedEncoder) {
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
            for &transition_id in self.transitions.of_source(source) {
                let transition_grounding =
                    self.transitions
                        .get_terms_eval_in_ground_source(transition_id, source_grounding, ctx);
                let transition_grounding_id = TransitionGroundingId {
                    state_var_grounding_id: self.state_vars_ground.post_ground_state_var(
                        &self.transitions.get_state_var(transition_id, ctx).fluent,
                        &transition_grounding.args,
                    ),
                    valfrom: transition_grounding.valfrom,
                    valto: transition_grounding.valto,
                };

                {
                    let transition_terms = self.transitions.get_terms(transition_id, ctx);

                    for (&term, &value) in transition_terms.args.iter().zip(&transition_grounding.args) {
                        if !term.is_cst() {
                            self.terms_ground.post_for_ground_transition(
                                term,
                                value,
                                transition_id,
                                transition_grounding_id,
                            );
                        }
                    }
                    if transition_terms.valfrom.is_some_and(|term| !term.is_cst()) {
                        self.terms_ground.post_for_ground_transition(
                            transition_terms.valfrom.unwrap(),
                            transition_grounding.valfrom.unwrap(),
                            transition_id,
                            transition_grounding_id,
                        );
                    }
                    if transition_terms.valto.is_some_and(|term| !term.is_cst()) {
                        self.terms_ground.post_for_ground_transition(
                            transition_terms.valto.unwrap(),
                            transition_grounding.valto.unwrap(),
                            transition_id,
                            transition_grounding_id,
                        );
                    }
                }

                self.transitions_ground.post_ground_transition(
                    transition_id,
                    transition_grounding_id,
                    source,
                    source_grounding_id,
                );
            }
        }
    }

    pub fn sort(&mut self) {
        self.supports.sort();
        self.transitions_ground.sort_for_all();
        self.supports_ground = SupportsGroundingsInfo::from(&self.supports, &self.transitions_ground);
        self.terms_ground.sort();
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
        let binding = ground_all_tasks(ctx);
        let sources_groundings = [(None, binding.empty_source_groundings().to_vec())].into_iter().chain(
            binding
                .all_task_groundings()
                .map(|(task, gs)| (Some(task), gs.to_vec())),
        );

        for (source, source_groundings) in sources_groundings {
            for source_grounding in source_groundings {
                self.post_ground_source(source, &source_grounding, ctx);
            }
        }

        self.sort();

        let mut problem = LpRelaxProblem::default();

        encode_problem_lifted(self, ctx, &mut problem);

        encode_problem_ground(self, ctx, &mut problem);

        problem
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{
        constraints::lprelax::{
            LpRelaxEncoder,
            examples::visitall::{VisitAllLine, build_and_encode_visitall_line},
            transitions::{Transition, Transitions, supports::Supports},
        },
        encoder::CausalLink,
    };

    #[test]
    fn test_supports() {
        let encoder = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 5,
                num_moves: 4,
            },
            false,
        );

        let transitions = Transitions::new_unambiguous(&encoder, true);
        let supports = Supports::from(&transitions, &encoder, None);
        let lprelax_encoder = LpRelaxEncoder {
            transitions,
            supports,
            sources_ground: Default::default(),
            state_vars_ground: Default::default(),
            transitions_ground: Default::default(),
            terms_ground: Default::default(),
            supports_ground: Default::default(),
        };

        for &((out_transition_id, in_transition_id), active) in lprelax_encoder.supports.unsorted_out() {
            if let Some(active) = active {
                assert!(
                    lprelax_encoder.transitions.is_pure_cond(in_transition_id)
                        || lprelax_encoder.transitions.is_condeff(in_transition_id)
                );

                assert!({
                    let (eff_id, cond_id) = match (
                        lprelax_encoder.transitions.get(out_transition_id),
                        lprelax_encoder.transitions.get(in_transition_id),
                    ) {
                        (
                            Transition::Eff(eff_id) | Transition::CondEff(_, eff_id),
                            Transition::Cond(cond_id) | Transition::CondEff(cond_id, _),
                        ) => (eff_id, cond_id),
                        _ => unreachable!(),
                    };
                    encoder.causal_links.get_links().contains(&CausalLink {
                        eff_id,
                        cond_id,
                        active,
                    })
                });
            } else {
                assert!(lprelax_encoder.transitions.is_pure_eff(in_transition_id));
                assert!(
                    lprelax_encoder
                        .transitions
                        .get_source(in_transition_id, &encoder)
                        .is_some()
                );

                let (out_eff_id, in_eff_id) = match (
                    lprelax_encoder.transitions.get(out_transition_id),
                    lprelax_encoder.transitions.get(in_transition_id),
                ) {
                    (Transition::Eff(out_eff_id) | Transition::CondEff(_, out_eff_id), Transition::Eff(in_eff_id)) => {
                        (out_eff_id, in_eff_id)
                    }
                    _ => unreachable!(),
                };

                assert!(
                    !lprelax_encoder
                        .transitions
                        .is_effect_recovered_missing_initial(in_eff_id)
                );

                if lprelax_encoder
                    .transitions
                    .is_effect_recovered_missing_initial(out_eff_id)
                {
                    assert!(lprelax_encoder.transitions.is_pure_eff(out_transition_id));
                    assert!(
                        lprelax_encoder
                            .transitions
                            .get_source(out_transition_id, &encoder)
                            .is_none()
                    );
                }
            }
        }
    }
}
