use std::collections::HashMap;

use aries_solver::core::views::Dom;
use aries_solver::prelude::*;
use idmap::intid::IntegerId;
use itertools::Itertools;

use crate::encoder::SchedEncoder;
use crate::ext::lprelax::encoding::{LpRelaxEncoding, LpRelaxProblem};
use crate::ext::lprelax::transitions::*;
use crate::ext::{Source, SourceGrounding};
use crate::{Effect, EffectId, IntTerm, Task, TaskId};

#[derive(Clone)]
pub(crate) struct LpRelaxEncoder {
    pub(crate) transitions: Transitions,
    // sources_grounder: todo!();
}

impl LpRelaxEncoder {
    pub fn new(ctx: &mut SchedEncoder) -> Self {
        let transitions = Transitions::new_unambiguous(ctx, super::ARIES_LPRELAX_RECOVER_MIES.get());

        Self { transitions }
    }

    pub fn get_source<'a>(&self, source: Source, ctx: &'a SchedEncoder) -> Option<&'a Task> {
        source.map(|task_id| &ctx.sched.tasks[task_id])
    }
    pub fn get_source_terms<'a>(&self, source: Source, ctx: &'a SchedEncoder) -> &'a [IntTerm] {
        source
            .map(|task_id| &ctx.sched.tasks[task_id].args)
            .unwrap_or(&ctx.sched.global_args)
    }

    // pub fn get_effect<'a>(&self, eff_id: EffectId, ctx: &'a SchedEncoder) -> &'a Effect {
    //     ctx.sched.effects.get(eff_id)
    // }
    // pub fn get_condition<'a>(&self, cond_id: CondId, ctx: &'a SchedEncoder) -> &'a HasValueAt {
    //     ctx.causal_links.conditions.get(cond_id)
    // }

    #[allow(dead_code)]
    pub fn get_transition(&self, transition_id: TransitionId) -> Transition {
        self.transitions.get(transition_id)
    }

    pub fn get_transition_terms<'a>(
        &'a self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> TransitionTermsView<'a> {
        self.transitions.get_terms(transition_id, ctx)
    }
    pub fn get_transition_terms_terms_eval_in_ground_source(
        &self,
        transition_id: TransitionId,
        source_grounding: &SourceGrounding,
        ctx: &SchedEncoder,
    ) -> TransitionGrounding {
        self.transitions
            .get_terms_eval_in_ground_source(transition_id, source_grounding, ctx)
    }

    pub fn iter_transitions(&self) -> impl Iterator<Item = (TransitionId, Transition)> {
        self.transitions.iter()
    }
    pub fn iter_effects<'a>(&'a self, ctx: &'a SchedEncoder) -> impl Iterator<Item = (EffectId, &'a Effect)> {
        self.transitions
            .iter_of_effects()
            .map(|(_, transition_id)| self.transitions.get_effect(transition_id, ctx).unwrap())
    }
    pub fn iter_sources(&self, ctx: &SchedEncoder) -> impl Iterator<Item = Source> {
        std::iter::chain(
            [None],
            ctx.sched
                .tasks
                .iter()
                .enumerate()
                .map(|(task_id, _)| Some(TaskId::from_int(u32::try_from(task_id).unwrap()))),
        )
    }
    /// Collect lifted and ground supports between transitions.
    /// Note that in the LP relaxation, effect transitions are allowed to
    /// be supporters of other effect transitions (on the same predicate / state function),
    /// which is not the case for causal links in the main encoding.
    /// In this specific case where the support is between two effects,
    /// the "active" literal is None (as this doesn't correspond to a causal link in the main CSP model).
    pub fn iter_supports<'a>(
        &'a self,
        ctx: &'a SchedEncoder,
    ) -> impl Iterator<Item = ((TransitionId, TransitionId), Option<Lit>)> {
        // Supporting stemming from the causal links in the main encoding.
        let supports_causal_links = ctx.causal_links.get_links().map(|cl| {
            // println!(
            //     "{:?} {:?}",
            //     (cl.eff_id, ctx.sched.effects.get(cl.eff_id)),
            //     (cl.cond_id, ctx.causal_links.conditions.get(cl.cond_id))
            // );
            let out_transition_id = self.transitions.of_effect(cl.eff_id).unwrap();
            let in_transition_id = self.transitions.of_condition(cl.cond_id).unwrap();

            debug_assert_eq!(
                self.transitions.get_state_var(out_transition_id, ctx).fluent,
                self.transitions.get_state_var(in_transition_id, ctx).fluent,
            );
            ((out_transition_id, in_transition_id), Some(cl.active))
        });

        // Supports from effects (including "missing" ones) to other effects (non-initial) effects
        let supports_from_effects_to_others = self.iter_effects(ctx).flat_map(move |(out_eff_id, _)| {
            let out_transition_id = self.transitions.of_effect(out_eff_id).unwrap();

            self.iter_effects(ctx).flat_map(move |(in_eff_id, _)| {
                let in_transition_id = self.transitions.of_effect(in_eff_id).unwrap();

                if out_transition_id == in_transition_id
                    || self.transitions.is_condeff(in_transition_id)
                    || self.transitions.get_source(in_transition_id, ctx).is_none()
                {
                    return None;
                }

                debug_assert!(
                    out_transition_id != in_transition_id
                        && self.transitions.is_pure_eff(in_transition_id)
                        && self.transitions.get_source(in_transition_id, ctx).is_some()
                );

                if self.transitions.get_state_var(out_transition_id, ctx).fluent
                    == self.transitions.get_state_var(in_transition_id, ctx).fluent
                {
                    let out_args = self.get_transition_terms(out_transition_id, ctx).args;
                    let in_args = self.get_transition_terms(in_transition_id, ctx).args;

                    if out_args.iter().zip(in_args).any(|(out_term, in_term)| {
                        out_term.is_cst() && in_term.is_cst() && out_term.constant != in_term.constant
                    }) {
                        None
                    } else {
                        Some(((out_transition_id, in_transition_id), None))
                    }
                } else {
                    None
                }
            })
        });

        // Supports from "missing" initial effects to anything other than an effect is not needed
        // (they otherwise wouldn't have been ignored in the main encoding).
        // As such, WARNING: This assumes that the main encoding soundly omits initial effects that may not support the conditions.

        (supports_causal_links.chain(supports_from_effects_to_others))
            .filter(|&((out_transition_id, in_transition_id), _)| out_transition_id != in_transition_id)
            .inspect(|&((out_transition_id, in_transition_id), _)| {
                debug_assert!(!self.transitions.is_pure_cond(out_transition_id));
                debug_assert!(
                    !self.transitions.is_pure_eff(in_transition_id)
                        || self.transitions.get_source(in_transition_id, ctx).is_some()
                );
            })
    }

    // TODO: complete / incomplete grounder ?
    #[allow(unused)]
    pub fn run_new_brutal_grounder(&self, ctx: &SchedEncoder) -> HashMap<Option<TaskId>, Vec<SourceGrounding>> {
        let mut res = HashMap::default();
        for source in self.iter_sources(ctx) {
            res.insert(
                source,
                self.get_source_terms(source, ctx)
                    .iter()
                    .map(|t| ctx.bounds(t).0..=ctx.bounds(t).1)
                    .multi_cartesian_product()
                    .map(SourceGrounding)
                    .collect(),
            );
        }
        res
    }
    // TODO: complete / incomplete grounder ?
    fn run_new_simple_datalog_grounder(&self, ctx: &SchedEncoder) -> Vec<(Source, Vec<SourceGrounding>)> {
        let time = std::time::Instant::now();
        println!("|- Datalog grounder started");

        let res = crate::ext::ground::SourcesGrounderSimple::from(ctx).run();

        for (source, source_groundings) in &res {
            println!(
                "|--- {source:?}: {} {}",
                source_groundings.len(),
                if true {
                    format!("{:?}", self.get_source(*source, ctx))
                } else {
                    "".to_string()
                }
            );
            // for grd in grds {
            //     println!("|    {grd:?}");
            // }
        }
        println!("|- Datalog grounder ended (run time: {})", time.elapsed().as_secs_f64());

        res
    }

    pub fn encode(&self, ctx: &SchedEncoder) -> (LpRelaxEncoding, LpRelaxProblem) {
        let mut encoding = LpRelaxEncoding::default();

        let sources_groundings = self.run_new_simple_datalog_grounder(ctx);

        let time = std::time::Instant::now();
        println!("|- Source groundings interning started");

        for (source, source_groundings) in sources_groundings {
            for source_grounding in source_groundings {
                encoding.post_ground_source(source, source_grounding, self, ctx);
            }
        }
        println!(
            "|- Source groundings interning ended (run time: {})",
            time.elapsed().as_secs_f64()
        );

        let problem = encoding.build(self, ctx);

        println!(
            "|- LPrelax problem stats: {} columns, {} rows",
            problem.cols().len(),
            problem.rows().len(),
        );

        (encoding, problem)
    }
}
