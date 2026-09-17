use aries_solver::lang::Lit;

use crate::{
    encoder::SchedEncoder,
    ext::lprelax::{
        LpRelaxEncoder,
        transitions::{TransitionId, Transitions},
    },
};

#[derive(Clone, Default)]
pub struct Supports {
    unsorted_out: Vec<((TransitionId, TransitionId), Option<Lit>)>,
    sorted: SupportsSorted,
}
impl Supports {
    pub fn from(transitions: &Transitions, ctx: &SchedEncoder, doms: Option<&crate::Domains>) -> Self {
        let iter_effects_info = || {
            transitions
                .iter_of_effects()
                .map(|(_, transition_id)| transitions.get_effect_info(transition_id, ctx).unwrap())
        };

        // Supporting stemming from the causal links in the main encoding.
        let supports_causal_links = ctx.causal_links.get_links().filter_map(|cl| {
            // println!(
            //     "{:?} {:?}",
            //     (cl.eff_id, ctx.sched.effects.get(cl.eff_id)),
            //     (cl.cond_id, ctx.causal_links.conditions.get(cl.cond_id))
            // );
            let out_transition_id = transitions.of_effect(cl.eff_id)?;
            let in_transition_id = transitions.of_condition(cl.cond_id)?;

            debug_assert_eq!(
                transitions.get_state_var(out_transition_id, ctx).fluent,
                transitions.get_state_var(in_transition_id, ctx).fluent,
            );
            Some(((out_transition_id, in_transition_id), Some(cl.active)))
        });

        // Supports from effects (including "missing" ones) to other effects (non-initial) effects
        let supports_from_effects_to_others = iter_effects_info().flat_map(move |(out_eff_id, _)| {
            let out_transition_id = transitions.of_effect(out_eff_id).unwrap();

            iter_effects_info().flat_map(move |(in_eff_id, _)| {
                let in_transition_id = transitions.of_effect(in_eff_id).unwrap();

                if out_transition_id == in_transition_id
                    || transitions.is_condeff(in_transition_id)
                    || transitions.get_source(in_transition_id, ctx).is_none()
                {
                    return None;
                }

                debug_assert!(
                    out_transition_id != in_transition_id
                        && transitions.is_pure_eff(in_transition_id)
                        && transitions.get_source(in_transition_id, ctx).is_some()
                );

                if transitions.get_state_var(out_transition_id, ctx).fluent
                    == transitions.get_state_var(in_transition_id, ctx).fluent
                {
                    let out_args = transitions.get_terms(out_transition_id, ctx).args;
                    let in_args = transitions.get_terms(in_transition_id, ctx).args;

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

        let unsorted_out = (supports_causal_links.chain(supports_from_effects_to_others))
            .filter(|&((out_transition_id, in_transition_id), _)| out_transition_id != in_transition_id)
            .inspect(|&((out_transition_id, in_transition_id), _)| {
                debug_assert!(!transitions.is_pure_cond(out_transition_id));
                debug_assert!(
                    !transitions.is_pure_eff(in_transition_id)
                        || transitions.get_source(in_transition_id, ctx).is_some()
                );
            })
            .collect();

        Self {
            unsorted_out,
            sorted: Default::default(),
        }
    }

    pub fn sort(&mut self) {
        let time = std::time::Instant::now();
        println!("|- Sorted lifted supports building started)");

        let mut sorted_out = Vec::with_capacity(self.unsorted_out.len());
        let mut sorted_in = Vec::with_capacity(self.unsorted_out.len());

        for &((out_transition_id, in_transition_id), active) in &self.unsorted_out {
            /*if let Some(doms) = doms
                && (doms.entails(!encoder.transitions.get_prez(out_transition_id, ctx))
                    || doms.entails(!encoder.transitions.get_prez(in_transition_id, ctx)))
            {
                continue;
            }*/
            sorted_out.push(((out_transition_id, in_transition_id), active));
            sorted_in.push((in_transition_id, out_transition_id));
        }
        sorted_out.sort_unstable();
        sorted_in.sort_unstable();

        println!(
            "|- Sorted lifted supports building ended (run time: {})",
            time.elapsed().as_secs_f64()
        );

        self.sorted.sorted_out = sorted_out;
        self.sorted.sorted_in = sorted_in;
    }

    pub fn unsorted_out(&self) -> &[((TransitionId, TransitionId), Option<Lit>)] {
        &self.unsorted_out
    }
    pub fn sorted_out(&self) -> &[((TransitionId, TransitionId), Option<Lit>)] {
        &self.sorted.sorted_out
    }
    pub fn sorted_in(&self) -> &[(TransitionId, TransitionId)] {
        &self.sorted.sorted_in
    }
}

#[derive(Clone, Default)]
pub(super) struct SupportsSorted {
    sorted_out: Vec<((TransitionId, TransitionId), Option<Lit>)>,
    sorted_in: Vec<(TransitionId, TransitionId)>,
}
