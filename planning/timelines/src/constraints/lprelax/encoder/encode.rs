use aries_linexpr::IntoLinExpr;
use aries_solver::core::IntCst;
use aries_solver::core::views::Dom;
use itertools::Itertools;

use crate::SchedEncoder;
use crate::lprelax::transitions::TransitionType;

use super::LpRelaxEncoder;
use super::problem::{ColTag, LinExpr, LpRelaxProblem, sum};

/// Pushes the "lifted rows"
pub(super) fn encode_problem_lifted(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    let supports_sorted = encoder.supports_sorted.as_ref().unwrap();

    // "The empty source must be present"
    problem.push_row(ColTag::PresenceSource(None, None).into_linexpr().eq(1));

    // "A transition's presence is equal (or sometimes leq) to its source's presence"
    {
        for (source, trans_ids) in encoder.iter_sources() {
            for &trans_id in trans_ids {
                // It is possible that sometimes the presence literal of the transition differs from that of the source
                // (in particular, as a result of complying with pddl set semantics,
                // if the transition presence literal was replaced with a more specific one than the source's).
                //
                // However, the transition's presence literal must always imply the source's.

                debug_assert!(ctx.store.state.implies(
                    encoder.transitions.get_prez(trans_id, ctx),
                    encoder.get_source_prez(source, ctx)
                ));
                let presences_equivalent = ctx.store.state.implies(
                    encoder.get_source_prez(source, ctx),
                    encoder.transitions.get_prez(trans_id, ctx),
                );

                let trans_presence = ColTag::PresenceTransition(trans_id, None).into_linexpr();
                let source_presence = ColTag::PresenceSource(source, None);

                // When the transition's and source's presence literals are truly equivalent, we can enforce equality.
                // Otherwise, we can only enforce one side (leq).

                problem.push_row(if presences_equivalent {
                    trans_presence.eq(source_presence)
                } else {
                    trans_presence.leq(source_presence)
                });
            }
        }
    }

    // In-support: "The presence of a transition equals the total support it receives / into it"
    //
    // NOTE: For effs, the equality becomes a geq, as they may also be supported by the initial state,
    //       which is only modeled at the ground level (see the ground in-support rows).
    {
        for (in_trans_id, out_trans_ids) in supports_sorted.iter_by_in() {
            let is_eff = encoder.transitions.get(in_trans_id).tpe() == TransitionType::Eff;

            let presence = ColTag::PresenceTransition(in_trans_id, None).into_linexpr();
            let supports = sum(out_trans_ids.map(|out_trans_id| ColTag::Support(out_trans_id, in_trans_id, None)));

            problem.push_row(if is_eff {
                presence.geq(supports)
            } else {
                presence.eq(supports)
            });
        }
    }

    // Out-support: "The presence of a transition is at least the total support it sends / hands over / from it"
    //
    // NOTE: When conds aren't allowed to be out-transitions / supporters, this total out-going support
    //       *must not* include support into conds. Indeed, without that, some effs could need to support multiple conds,
    //       which would be forbidden with support into conds included in the sum.
    {
        for (out_trans_id, in_trans_ids) in supports_sorted.iter_by_out() {
            let supports = in_trans_ids
                .filter(|&(in_trans_id, _)| {
                    encoder.supports.with_condition_out_transitions
                        || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
                })
                .map(|(in_trans_id, _)| ColTag::Support(out_trans_id, in_trans_id, None));
            let supports = sum(supports);

            if !supports.terms().is_empty() {
                let presence = ColTag::PresenceTransition(out_trans_id, None);

                problem.push_row(presence.into_linexpr().geq(supports));
            }
        }
    }

    // "A support implies (i.e. is leq to) the presence of its out-transition"
    //
    // NOTE: This constraint is explicitly needed *only when* conds aren't allowed to be out-transitions / supporters.
    //       Indeed, the out-support constraints would otherwise imply them.
    if !encoder.supports.with_condition_out_transitions {
        for &((out_trans_id, in_trans_id), _) in encoder.supports.unsorted_out() {
            if encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond {
                let support = ColTag::Support(out_trans_id, in_trans_id, None);
                let presence = ColTag::PresenceTransition(out_trans_id, None);

                problem.push_row(support.into_linexpr().leq(presence));
            }
        }
    }
}

/// Pushes the "lifted-ground" or "ground rows"
pub(super) fn encode_problem_ground(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    let ground = &encoder.ground;
    let with_initial_state_supports = crate::lprelax::ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS.get();
    let with_achievers = crate::lprelax::ARIES_LPRELAX_WITH_ACHIEVERS.get();
    let (with_value_graph_flow, with_support_graph_flow) =
        match crate::lprelax::ARIES_LPRELAX_WITH_FLOW_CONSTRS.get_ref().as_str() {
            "none" => (false, false),
            "valgraph" => (true, false),
            "supgraph" => (false, true),
            other => panic!("unknown ARIES_LPRELAX_WITH_FLOW_CONSTRS: {other} (expected none, valgraph or supgraph)"),
        };

    // Ground decomposition: "A source's presence equals the total presence of its groundings".
    {
        for (source, _) in encoder.iter_sources() {
            let source_groundings = ground.source_groundings(source);
            if source_groundings.is_empty() {
                // No grounding: the source can't be present (the main model restricts its parameters to its groundings),
                // which is left to the main model here. (To revisit with partial groundings.)
                continue;
            }
            let groundings = source_groundings
                .iter()
                .map(|&g| ColTag::PresenceSource(source, Some(g)));

            problem.push_row(ColTag::PresenceSource(source, None).into_linexpr().eq(sum(groundings)));
        }
    }

    // Ground decomposition: "A transition's presence equals the total presence of its groundings".
    {
        for (trans_id, groundings) in ground.iter_transition_groundings() {
            let groundings = sum(groundings.map(|g| ColTag::PresenceTransition(trans_id, Some(g))));

            problem.push_row(ColTag::PresenceTransition(trans_id, None).into_linexpr().eq(groundings));
        }
    }

    // "A ground transition's presence equals the total presence of its source's compatible groundings"
    {
        for (trans_id, trans_grounding_id, source, source_groundings) in
            ground.iter_transition_groundings_with_sources()
        {
            let presence = ColTag::PresenceTransition(trans_id, Some(trans_grounding_id)).into_linexpr();
            let source_presences = sum(source_groundings.map(|g| ColTag::PresenceSource(source, Some(g))));

            debug_assert!(ctx.store.state.implies(
                encoder.transitions.get_prez(trans_id, ctx),
                encoder.get_source_prez(source, ctx)
            ));
            let presences_equivalent = ctx.store.state.implies(
                encoder.get_source_prez(source, ctx),
                encoder.transitions.get_prez(trans_id, ctx),
            );

            // Same as for the lifted rows: the transition's presence literal must always imply the source's,
            // and we can enforce equality only when they are equivalent.

            problem.push_row(if presences_equivalent {
                presence.eq(source_presences)
            } else {
                presence.leq(source_presences)
            });
        }
    }

    // Ground decomposition: "A support is equal to the total support of its groundings"
    {
        for (out_trans_id, in_trans_id, support_groundings) in ground.iter_support_groundings() {
            let ground_supports = support_groundings.map(|g| ColTag::Support(out_trans_id, in_trans_id, Some(g)));
            let support = ColTag::Support(out_trans_id, in_trans_id, None);

            problem.push_row(support.into_linexpr().eq(sum(ground_supports)));
        }
    }

    // In-support: "The total presence of a transition's groundings with a same ground state variable and pre-value
    //              equals the total ground support they receive"
    //
    // NOTE: For effs, this includes the support from the initial state of the ground state variable
    //       (or, when it isn't modeled, the equality becomes a geq).
    {
        for (in_trans_id, (state_var_grounding_id, val_assignment), groundings, supports) in
            ground.iter_support_in_groups()
        {
            let is_eff = val_assignment.is_none();

            let presences = sum(groundings.map(|g| ColTag::PresenceTransition(in_trans_id, Some(g))));
            let supports = supports.map(|(out_trans_id, support_grounding_id)| {
                ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id))
            });

            // An eff may be supported by the initial state.
            let support_from_initial_state = (with_initial_state_supports && is_eff)
                .then_some(ColTag::SupportFromInitialState(state_var_grounding_id, in_trans_id));
            let supports = sum(supports.chain(support_from_initial_state));

            problem.push_row(if !with_initial_state_supports && is_eff {
                presences.geq(supports)
            } else {
                presences.eq(supports)
            });
        }
    }

    // "The initial state of a ground state variable supports at most one eff"
    if with_initial_state_supports {
        for (state_var_grounding_id, effs) in ground.iter_effects_by_state_var() {
            // Implied by the column's bounds when a single eff is concerned.
            if effs.len() <= 1 {
                continue;
            }
            let supports = effs
                .iter()
                .map(|&trans_id| ColTag::SupportFromInitialState(state_var_grounding_id, trans_id));

            problem.push_row(ColTag::PresenceSource(None, None).into_linexpr().geq(sum(supports)));
        }
    }

    // Out-support: "The total presence of a transition's groundings handing over a same value on a same ground state variable
    //               is at least the total ground support they hand over"
    //
    // NOTE: Same as for the lifted rows: support into conds is left out when conds aren't allowed to be out-transitions / supporters.
    {
        for (out_trans_id, support_grounding_id, groundings, in_trans_ids) in ground.iter_support_out_groups() {
            let supports = in_trans_ids
                .filter(|&in_trans_id| {
                    encoder.supports.with_condition_out_transitions
                        || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
                })
                .map(|in_trans_id| ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id)));
            let supports = sum(supports);

            if supports.terms().is_empty() {
                continue;
            }
            let presences = sum(groundings.map(|g| ColTag::PresenceTransition(out_trans_id, Some(g))));

            problem.push_row(presences.geq(supports));
        }
    }

    // "A ground support implies (i.e. is leq to) the total presence of the out-transition's groundings it can come from"
    //
    // NOTE: Same as for the lifted rows: only needed when conds aren't allowed to be out-transitions / supporters.
    if !encoder.supports.with_condition_out_transitions {
        for (out_trans_id, support_grounding_id, groundings, in_trans_ids) in ground.iter_support_out_groups() {
            let presences = sum(groundings.map(|g| ColTag::PresenceTransition(out_trans_id, Some(g))));

            for in_trans_id in in_trans_ids {
                if encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond {
                    let support = ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id));

                    problem.push_row(support.into_linexpr().leq(presences.clone()));
                }
            }
        }
    }

    // For terms used by a single source, with table constraints for its groundings, the term rows below are left out (as well as their bindings).
    // (Only when the tables are enforced unconditionally: otherwise, the line literals of a present source may all be false.)
    let bound_by_table = |term| match (&ctx.grounding_tables, ground.term_single_source(term)) {
        (Some(tables), Some(Some(task))) if tables.enforced_if == aries_solver::core::Lit::TRUE => {
            tables.table_line_lits.contains_key(task)
        }
        _ => false,
    };

    // "A term takes at most one value (i.e. at most one of its groundings is active)"
    //
    // NOTE: Implied for a term appearing in a single source (by the rows below and the source's ground decomposition).
    for (term, values) in ground.iter_term_values() {
        if bound_by_table(term) {
            continue;
        }
        let groundings = sum(values.map(|value| ColTag::TermGround(term, value)));
        debug_assert!(!groundings.terms().is_empty());

        problem.push_row(groundings.leq(1));
    }

    // "A term grounding is active iff one of the groundings (with that value) of a source using the term is"
    for (term, value, source, source_groundings) in ground.iter_term_assignments_with_sources() {
        if bound_by_table(term) {
            continue;
        }
        let source_presences = sum(source_groundings.map(|g| ColTag::PresenceSource(source, Some(g))));

        debug_assert!(
            ctx.store
                .state
                .implies(encoder.get_source_prez(source, ctx), ctx.store.presence(term))
                && ctx
                    .store
                    .state
                    .implies(ctx.store.presence(term), encoder.get_source_prez(source, ctx))
        );

        problem.push_row(ColTag::TermGround(term, value).into_linexpr().eq(source_presences));
    }

    // Support graph flow: "A present ground transition must be reachable from the initial state (for its ground state variable), through active ground supports"
    //
    // NOTE: These constraints correspond to a single-commodity flow formulation over the "support graph" of each ground state variable:
    //       its nodes are the ground transitions, plus the initial state for each ground state variable,
    //       and its arcs are the (ground) supports between them (arcs / supports from the initial state only enter nodes corresponding to eff-transitions).
    //
    //       The constraints are of two types: capacity and balance.
    //       - Capacity: the flow along an arc is at most big-M times the total of its ground supports
    //         (so this flow is 0 unless one of them is active).
    //         Big-M is -- for a given ground state variable -- the number of transitions that can be grounded to it,
    //         i.e. the maximum total flow they may consume (as a transition's groundings are mutually exclusive).
    //       - Balance: the flow into a transition (node) minus the flow out of it equals the total presence of its groundings
    //         (on that ground state variable), i.e. the amount of flow this transition consumes.
    //
    //       As flow only enters the graph at the initial state, it must reach every present transition through active supports,
    //       which rules out cycles of supports. However, with big-M, fractional cycles may still slip through.
    //
    // NOTE: Without initial-state supports, effs may be unsupported,
    //       so they would need to become the flow's sources instead of the initial state (they get no balance row).
    //       This would weaken these constraints a bit, as cycles of supports through an eff wouldn't be tackled.
    if with_support_graph_flow {
        // The big-M of each ground state variable (see above), by id (as the ids are contiguous, from 0).
        let big_ms = ground
            .iter_transition_groundings_by_state_var()
            .enumerate()
            .map(|(i, (state_var_grounding_id, entries))| {
                debug_assert_eq!(i, state_var_grounding_id);
                entries.iter().map(|&(_, trans_id)| trans_id).unique().count() as IntCst
            })
            .collect::<Vec<_>>();

        // The terms of the balance rows (see below): ((transition, ground state variable), column, coefficient).
        let mut balance_terms = vec![];

        // The flow consumed by the transitions: the presences of their groundings on each ground state variable.
        for (state_var_grounding_id, entries) in ground.iter_transition_groundings_by_state_var() {
            for &(g, trans_id) in entries {
                balance_terms.push((
                    (trans_id, state_var_grounding_id),
                    ColTag::PresenceTransition(trans_id, Some(g)),
                    -1,
                ));
            }
        }

        // The arcs of the supports between transitions, on each ground state variable.
        for (out_trans_id, in_trans_id, support_groundings) in ground.iter_support_groundings() {
            // (The ground supports come sorted by ground state variable, then by value.)
            let support_groundings = support_groundings.collect::<Vec<_>>();
            for supports in support_groundings.chunk_by(|a, b| a.0 == b.0) {
                let state_var_grounding_id = supports[0].0;
                let flow = ColTag::SupportGraphFlow(Some(out_trans_id), in_trans_id, state_var_grounding_id);

                balance_terms.push(((out_trans_id, state_var_grounding_id), flow, -1));
                balance_terms.push(((in_trans_id, state_var_grounding_id), flow, 1));

                // Capacity: "A support's flow is at most big-M times the total of its ground supports"
                let capacity = supports
                    .iter()
                    .map(|&g| {
                        (
                            ColTag::Support(out_trans_id, in_trans_id, Some(g)),
                            big_ms[state_var_grounding_id],
                        )
                    })
                    .collect::<LinExpr>();

                problem.push_row(flow.into_linexpr().leq(capacity));
            }
        }

        // The arcs from the initial state of each ground state variable, into its effs.
        if with_initial_state_supports {
            for (state_var_grounding_id, effs) in ground.iter_effects_by_state_var() {
                for trans_id in effs {
                    let flow = ColTag::SupportGraphFlow(None, trans_id, state_var_grounding_id);

                    balance_terms.push(((trans_id, state_var_grounding_id), flow, 1));

                    // Capacity: "The flow from the initial state into an eff is at most big-M times the support between them"
                    let capacity = ColTag::SupportFromInitialState(state_var_grounding_id, trans_id)
                        * big_ms[state_var_grounding_id];

                    problem.push_row(flow.into_linexpr().leq(capacity));
                }
            }
        }

        // Balance: "The flow into a (ground) transition minus the flow out of it equals the total presence of its groundings on the ground state variable"
        //          (written with everything moved to the left-hand side)
        balance_terms.sort_unstable_by_key(|&(key, _, _)| key);
        for terms in balance_terms.chunk_by(|a, b| a.0 == b.0) {
            let (trans_id, _) = terms[0].0;
            if !with_initial_state_supports && encoder.transitions.get(trans_id).tpe() == TransitionType::Eff {
                continue; // (an eff is then a source of the flow, see above)
            }
            let row = terms.iter().map(|&(_, col, coef)| (col, coef)).collect::<LinExpr>();

            problem.push_row(row.eq(0));
        }
    }

    // Value graph flow: "A present ground transition's pre-value should be reachable from `None` (i.e. from an eff), through value changes made by present ground transitions"
    //
    // NOTE: These constraints correspond to a single-commodity flow formulation over the "value graph" of each ground state variable.
    //       Its nodes are the state variable's values and `None`, and its arcs are the value changes made by ground transitions,
    //       from their pre-value (`None` for effs) to their post-value.
    //       This value graph is similar to a domain transition graph (DTG), which uses (ground) actions as arcs instead of transition groundings.
    //
    //       The constraints are of two types: capacity and balance.
    //       - Capacity: the flow along an arc is at most big-M times the total presence of the ground transitions corresponding to the arc
    //         (so this flow is 0 unless one of these ground transitions is present).
    //         Big-M is -- for a given ground state variable -- the number of transitions that have a concrete pre-value (so just non-eff transitions),
    //         i.e. the maximum total flow that could pass through an arc (for the considered ground state variable).
    //       - Balance: the flow into a value (node) minus the flow out of it equals the total presence of the ground transitions with that (pre-)value.
    //         In other words, this difference is the amount of flow these ground transitions consume (whether they actually change the value or not).
    //
    //       As flow only enters the graph at the `None` node (which only effs have as their pre-value),
    //       whatever is consumed at a concrete value must come from effs through value changes of present ground transitions.
    //       These constraints (with the fact that flow is non-negative) help rule out cycles of transitions involving values into which no flow enters,
    //       by forcing the flow (and thus the "consumption budget" of presences) to "monotonically decrease" along the way.
    //       However, with big-M, a small fraction of a ground transition's presence can carry a lot of flow, so fractional cycles may still slip through.
    if with_value_graph_flow {
        for (_, entries) in ground.iter_transition_groundings_by_state_var() {
            // The big-M of the capacities:
            // the number of transitions with a concrete pre-value on the state variable,
            // each taking in at most 1 (presence upper bound) unit of flow (as a transition's groundings are mutually exclusive).
            let big_m = entries
                .iter()
                .filter(|(g, _)| g.pre_val_assignment.is_some())
                .map(|&(_, trans_id)| trans_id)
                .unique()
                .count() as IntCst;
            if big_m == 0 {
                continue;
            }

            // The terms of the balance rows (see below): (value, column, coefficient).
            let mut balance_terms = vec![];

            // (The entries come sorted by grounding, i.e. by arc: ground state variable, pre-value and post-value.)
            for arc_entries in entries.chunk_by(|a, b| a.0 == b.0) {
                let g = arc_entries[0].0;
                let presences = arc_entries
                    .iter()
                    .map(|&(g, trans_id)| ColTag::PresenceTransition(trans_id, Some(g)));

                if let Some(pre_val) = g.pre_val_assignment {
                    balance_terms.extend(presences.clone().map(|presence| (pre_val, presence, -1)));
                }
                if g.pre_val_assignment == g.post_op_assignment {
                    continue; // (ground condeff-transition doesn't actually change the value)
                }
                let flow = ColTag::ValueGraphFlow(g);

                balance_terms.extend(g.pre_val_assignment.map(|pre_val| (pre_val, flow, -1)));
                balance_terms.extend(g.post_op_assignment.map(|post_val| (post_val, flow, 1)));

                // Capacity: "An arc's flow is at most big-M times the total presence of the ground transitions corresponding to that arc"
                let capacity = presences.map(|presence| (presence, big_m)).collect::<LinExpr>();

                problem.push_row(flow.into_linexpr().leq(capacity));
            }

            // Balance: "The flow into a value minus the flow out of it equals the total presence of the ground transitions with that (pre-)value"
            //          (written with everything moved to the left-hand side)
            balance_terms.sort_unstable_by_key(|&(value, _, _)| value);
            for value_terms in balance_terms.chunk_by(|a, b| a.0 == b.0) {
                let row = value_terms
                    .iter()
                    .map(|&(_, col, coef)| (col, coef))
                    .collect::<LinExpr>();

                problem.push_row(row.eq(0));
            }
        }
    }

    // Achievers: "The total presence of the ground transitions 'entering' a value on a ground state variable
    //             is at least the total presence of each transition's groundings with that pre-value (on that ground state variable, and for every transition)"
    //
    // NOTE: A ground transition "enters" a value `v` if it has it as post-value, but not as pre-value
    //       (i.e. the ground transitions must be of the form eff := `v` or condeff : `v'` -> `v` (with `v'` != `v`)).
    //       In a plan, a present ground transition's pre-value must be entered by a present ground transition.
    //       So these rows rule out cycles of ground transitions passing on a value that no ground transition enters,
    //       even when it's entered by a small fraction of a ground transition's presence let through due to the big-M of flow capacity rows !
    if with_achievers {
        // The presences of the ground transitions entering each value: ((ground state variable, value), presence).
        let mut entering = vec![];
        for (state_var_grounding_id, entries) in ground.iter_transition_groundings_by_state_var() {
            for &(g, trans_id) in entries {
                if let Some(post_val) = g.post_op_assignment
                    && g.pre_val_assignment != Some(post_val)
                {
                    entering.push((
                        (state_var_grounding_id, post_val),
                        ColTag::PresenceTransition(trans_id, Some(g)),
                    ));
                }
            }
        }
        entering.sort_unstable_by_key(|&(key, _)| key);

        // The values that are pre-values (on each ground state variable).
        let mut pre_vals = vec![];

        for (in_trans_id, (state_var_grounding_id, pre_val), groundings, _) in ground.iter_support_in_groups() {
            // (An eff has no pre-value.)
            let Some(pre_val) = pre_val else {
                continue;
            };
            pre_vals.push((state_var_grounding_id, pre_val));

            let achievers = ColTag::AuxAchievers(state_var_grounding_id, pre_val);
            let presences = sum(groundings.map(|g| ColTag::PresenceTransition(in_trans_id, Some(g))));

            problem.push_row(achievers.into_linexpr().geq(presences));
        }

        // "A value's achievers column is the total presence of the ground transitions entering it"
        pre_vals.sort_unstable();
        pre_vals.dedup();
        for key in pre_vals {
            let start = entering.partition_point(|&(k, _)| k < key);
            let presences = entering[start..]
                .iter()
                .take_while(|&&(k, _)| k == key)
                .map(|&(_, presence)| presence);
            let (state_var_grounding_id, pre_val) = key;
            let achievers = ColTag::AuxAchievers(state_var_grounding_id, pre_val);

            problem.push_row(achievers.into_linexpr().eq(sum(presences)));
        }
    }
}
