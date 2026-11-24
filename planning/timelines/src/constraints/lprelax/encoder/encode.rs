use aries_linexpr::IntoLinExpr;
use aries_solver::core::IntCst;
use aries_solver::core::state::Domains;
use itertools::Itertools;

use crate::SchedEncoder;
use crate::lprelax::transitions::{Transition, TransitionId, TransitionType};

use super::LpRelaxEncoder;
use super::ground::{StateVarGroundingId, TransitionGroundingId};
use super::problem::{ColTag, LinExpr, LpRelaxProblem, sum};

/// Pushes the "lifted rows"
pub(super) fn encode_problem_lifted(
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    problem: &mut LpRelaxProblem,
) {
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

                debug_assert!(doms.implies(
                    encoder.transitions.get_prez(trans_id, ctx),
                    encoder.get_source_prez(source, ctx)
                ));
                let presences_equivalent = doms.implies(
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

    if crate::lprelax::ARIES_LPRELAX_WITH_SYMMETRY_BREAKING.get() {
        assert!(
            !encoder.supports.with_condition_out_transitions,
            "the symmetry breaking rows (ARIES_LPRELAX_WITH_SYMMETRY_BREAKING=true) need conds not as out-transitions (ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS=false)"
        );
        encode_symmetry_breaking_rows(encoder, ctx, problem);
    }
}

// Symmetry breaking: linearizes the causal-graph symmetry breaking of the main model (see `crate::constraints::symmetry`).
//
// NOTE: For two consecutive interchangeable tasks `b` and `a` (`b` coming first), the main model enforces, for each item `i`
//       of their support signatures, the clause `a_i <= b_i or (a_j < b_j for some j < i)` (ECAI 25, eq. 5),
//       where `a_j < b_j` is `b_j` when the two are exclusive (and implies it otherwise), and `a_i <= b_i` is `not a_i` when they are exclusive.
//       So the clause implies the lifted row `a_i <= sum(b_j for j < i)` (`+ b_i` if not exclusive) over the support columns of the items.
//
//       The support columns are those of the causal links only when conds aren't out-transitions.
fn encode_symmetry_breaking_rows(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    use crate::encoder::CausalLink;
    use hashbrown::HashSet;

    let causal_link_supports: HashSet<(TransitionId, TransitionId)> = (encoder.supports.unsorted_out().iter())
        .filter(|(_, active)| active.is_some())
        .map(|&(support, _)| support)
        .collect();
    // The support column of a causal link: `Some(None)` if the link can never be active (it has no column, i.e. it is 0),
    // and `None` if the LP doesn't model the link (its value is unknown).
    let support_col = |cl: &CausalLink| -> Option<Option<ColTag>> {
        let out_trans_id = encoder.transitions.of_effect(cl.eff_id)?;
        let in_trans_id = encoder.transitions.of_condition(cl.cond_id)?;
        Some(
            causal_link_supports
                .contains(&(out_trans_id, in_trans_id))
                .then_some(ColTag::Support(out_trans_id, in_trans_id, None)),
        )
    };

    let mut num_rows = 0;
    for signature in &ctx.symmetry_signatures {
        // The support columns of the items of `b` so far
        let mut b_cols = vec![];
        for item in signature {
            let b = support_col(&item.b);
            if let Some(Some(a)) = support_col(&item.a) {
                let b_term = if item.exclusive { Some(None) } else { b };
                if let Some(b_term) = b_term {
                    problem.push_row(a.into_linexpr().leq(sum(b_cols.iter().copied().chain(b_term))));
                    num_rows += 1;
                }
            }
            match b {
                Some(b) => b_cols.extend(b),
                // (the rows of the next items would need its unknown value)
                None => break,
            }
        }
    }

    lprelax_log!("Symmetry breaking: {num_rows} lifted rows");
}

/// Pushes the "lifted-ground" or "ground rows"
pub(super) fn encode_problem_ground(
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    problem: &mut LpRelaxProblem,
) {
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
    let with_ground_supports = crate::lprelax::ARIES_LPRELAX_WITH_GROUND_SUPPORTS.get();
    let with_net_change_constrs = crate::lprelax::ARIES_LPRELAX_WITH_NET_CHANGE_CONSTRS.get();
    let with_goal_net_change_constrs = crate::lprelax::ARIES_LPRELAX_WITH_GOAL_NET_CHANGE_CONSTRS.get();
    assert!(
        with_ground_supports || !with_support_graph_flow,
        "the support graph flow (ARIES_LPRELAX_WITH_FLOW_CONSTRS=supgraph) needs the ground supports (ARIES_LPRELAX_WITH_GROUND_SUPPORTS=true)"
    );

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

            debug_assert!(doms.implies(
                encoder.transitions.get_prez(trans_id, ctx),
                encoder.get_source_prez(source, ctx)
            ));
            let presences_equivalent = doms.implies(
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
    if with_ground_supports {
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
    if with_ground_supports {
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
    if with_initial_state_supports && with_ground_supports {
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
    if with_ground_supports {
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
    if !encoder.supports.with_condition_out_transitions && with_ground_supports {
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
            doms.implies(encoder.get_source_prez(source, ctx), doms.presence(term))
                && doms.implies(doms.presence(term), encoder.get_source_prez(source, ctx))
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
    //       from their pre-value (`None` for effs) to their post-value (for non-erase effs).
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

    // Net change: "On a ground state variable, the total presence of the ground transitions leaving a value
    //              is at most the total presence of those entering it, minus 1 if it's the value of a goal (at the horizon)"
    //
    // NOTE: Each departure from a value needs its own earlier arrival, the initial state being an eff of the empty source.
    //       A goal at the horizon also needs its own arrival, never left afterwards
    //       (as the effects of actions take time (epsilon) and end before the horizon, every departure is before such a goal).
    //       Two goals on a same value can share their arrival, so there is one row per goal.
    //       (Corresponds to the net-change constraints of operator counting, with their goal term)
    //
    //       Erase effects count as departures.
    //
    //       The goal term isn't valid for goals before the horizon: a departure after the goal can use up the goal's arrival.
    //       Conditions of the empty source before the horizon are left as other conditions (i.e. out of these rows).
    //
    //       With only the goal rows, the rows of values without goals are left out.
    if with_net_change_constrs || with_goal_net_change_constrs {
        let is_goal_at_horizon = |trans_id: TransitionId| match encoder.transitions.get(trans_id) {
            Transition::Cond(cond_id) => {
                encoder.transitions.get_source(trans_id, ctx).is_none()
                    && ctx.causal_links.conditions.get(cond_id).period == ctx.sched.horizon.into()
            }
            Transition::Eff(_) | Transition::CondEff(..) => false,
        };
        for (_, entries) in ground.iter_transition_groundings_by_state_var() {
            // (value, goal, column, coefficient), with no goal for the arrivals and departures
            let mut terms = vec![];
            for &(g, trans_id) in entries {
                let presence = ColTag::PresenceTransition(trans_id, Some(g));
                if is_goal_at_horizon(trans_id) {
                    terms.push((g.pre_val_assignment.unwrap(), Some(trans_id), presence, -1));
                    continue;
                }
                if g.pre_val_assignment.is_some() && g.pre_val_assignment == g.post_op_assignment {
                    continue; // (doesn't change the value)
                }
                terms.extend(g.post_op_assignment.map(|post_val| (post_val, None, presence, 1)));
                terms.extend(g.pre_val_assignment.map(|pre_val| (pre_val, None, presence, -1)));
            }
            terms.sort_unstable_by_key(|&(value, goal, _, _)| (value, goal));
            for value_terms in terms.chunk_by(|a, b| a.0 == b.0) {
                let (changes, goal_terms) = value_terms.split_at(value_terms.partition_point(|t| t.1.is_none()));
                let changes = changes.iter().map(|&(_, _, col, coef)| (col, coef));
                if goal_terms.is_empty() && with_net_change_constrs {
                    if changes.clone().all(|(_, coef)| coef > 0) {
                        continue; // (never left)
                    }
                    problem.push_row(changes.clone().collect::<LinExpr>().geq(0));
                }
                for goal_terms in goal_terms.chunk_by(|a, b| a.1 == b.1) {
                    let goal_terms = goal_terms.iter().map(|&(_, _, col, coef)| (col, coef));

                    problem.push_row(changes.clone().chain(goal_terms).collect::<LinExpr>().geq(0));
                }
            }
        }
    }

    if crate::lprelax::ARIES_LPRELAX_WITH_GOAL_COMMODITIES.get() {
        encode_goal_commodities(encoder, ctx, doms, problem);
    }
}

// Goal commodities: "A goal's required value is reached from `None` (i.e. from an eff), through value changes made by present ground transitions"
//
// NOTE: One commodity per goal routes the goal's presence along the arcs of the value graphs (as for the value graph flow)
//       of the ground state variables its groundings require values on, each arc carrying at most the total presence
//       of its ground transitions (no big-M), into a virtual sink fed by the required value of each grounding,
//       at most the grounding's presence. With a single grounding, the sink is left out: the goal's presence is
//       the demand at its required value (the ground formulation).
//
//       A grounding's required value is its own value if its ground state variable is multi-valued (more than 2 values).
//       Otherwise, it comes from an "achiever link": every supporter of the goal is in a source with an eff or condeff,
//       on a single multi-valued ground state variable, whose post-value term is the supporter's state variable argument
//       at a same position (as `visited(?n)` and `at := ?n` in visit-all's move): the grounding requires the value of
//       its own state variable's argument at that position.
//       A goal is skipped if one of its groundings has no required value (as the sink must be able to receive from each).
//       Pure effs (other than the initial state's) let flow in at any value they assign, so they weaken these rows.
fn encode_goal_commodities(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, doms: &Domains, problem: &mut LpRelaxProblem) {
    use crate::EffectOp;
    use aries_solver::core::Lit;
    use hashbrown::HashMap;
    use std::collections::BTreeMap;

    let ground = &encoder.ground;

    // The ground transitions of each ground state variable (sorted by grounding, i.e. by arc).
    let by_state_var: HashMap<StateVarGroundingId, &[(TransitionGroundingId, TransitionId)]> =
        ground.iter_transition_groundings_by_state_var().collect();
    let num_values = |state_var: StateVarGroundingId| {
        (by_state_var.get(&state_var).copied().unwrap_or_default().iter())
            .flat_map(|(g, _)| [g.pre_val_assignment, g.post_op_assignment])
            .flatten()
            .unique()
            .count()
    };
    let groundings: HashMap<TransitionId, Vec<TransitionGroundingId>> = (ground.iter_transition_groundings())
        .map(|(trans_id, groundings)| (trans_id, groundings.collect()))
        .collect();
    // The ground state variable of all the groundings of a transition, if it's the same.
    let single_state_var = |trans_id: TransitionId| {
        (groundings.get(&trans_id)?.iter())
            .map(|g| g.state_var_grounding_id)
            .all_equal_value()
            .ok()
    };
    let post_term = |trans_id: TransitionId| match encoder.transitions.get(trans_id) {
        Transition::Eff(eff_id) | Transition::CondEff(_, eff_id) => match ctx.sched.effects.get(eff_id).operation {
            EffectOp::Assign(term) => Some(term),
            EffectOp::Step(_) => None,
            EffectOp::Erase => None,
        },
        Transition::Cond(_) => None,
    };
    let mut supporters: HashMap<TransitionId, Vec<TransitionId>> = HashMap::new();
    for &((out_trans_id, in_trans_id), _) in encoder.supports.unsorted_out() {
        supporters.entry(in_trans_id).or_default().push(out_trans_id);
    }
    // The achiever link of a goal, if any: the ground state variable, and the position of the goal's argument
    // whose value is assigned there by the goal's supporters' sources.
    let achiever_link = |goal: TransitionId| -> Result<(StateVarGroundingId, usize), &'static str> {
        let goal_supporters = supporters.get(&goal).map(|s| s.as_slice()).unwrap_or_default();
        if goal_supporters.is_empty() {
            return Err("no supporter");
        }
        if (goal_supporters.iter()).any(|&s| encoder.transitions.get_source(s, ctx).is_none()) {
            return Err("may be supported by the empty source (e.g. the initial state)");
        }
        let links = (goal_supporters.iter())
            .map(|&supporter| {
                let source = encoder.transitions.get_source(supporter, ctx);
                let args = &encoder.transitions.get_state_var(supporter, ctx).args;
                (encoder.transitions.of_source(source).iter())
                    .filter(|&&t| {
                        // (the transition must be present whenever the supporter is)
                        t != supporter
                            && doms.implies(
                                encoder.transitions.get_prez(supporter, ctx),
                                encoder.transitions.get_prez(t, ctx),
                            )
                    })
                    .find_map(|&t| {
                        let term = post_term(t)?;
                        let position = args.iter().position(|&arg| arg == term)?;
                        let state_var = single_state_var(t)?;
                        (num_values(state_var) > 2).then_some((state_var, position))
                    })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or("no achiever link")?;
        links
            .into_iter()
            .all_equal_value()
            .map_err(|_| "different achiever links")
    };

    // For each goal: its demand (its presence), and the required value of each of its groundings:
    // (grounding, ground state variable, value).
    let mut goals = vec![];
    let mut skipped: BTreeMap<&str, usize> = BTreeMap::new();
    for &goal in encoder.transitions.of_source(None) {
        if encoder.transitions.get(goal).tpe() != TransitionType::Cond {
            continue;
        }
        let Some(goal_groundings) = groundings.get(&goal).filter(|gs| !gs.is_empty()) else {
            *skipped.entry("ungrounded goal").or_default() += 1;
            continue;
        };
        let mut link = None;
        let targets = (goal_groundings.iter())
            .map(|&g| {
                let (state_var, value) = (g.state_var_grounding_id, g.pre_val_assignment.unwrap());
                if num_values(state_var) > 2 {
                    return Ok((g, state_var, value));
                }
                let (link_state_var, position) = *link
                    .get_or_insert_with(|| achiever_link(goal))
                    .as_ref()
                    .map_err(|&e| e)?;
                Ok((g, link_state_var, ground.state_var_args(state_var)[position]))
            })
            .collect::<Result<Vec<_>, &str>>();
        match targets {
            Ok(targets) => goals.push((goal, targets)),
            Err(reason) => *skipped.entry(reason).or_default() += 1,
        }
    }

    let (mut num_cols, mut num_rows) = (0, 0);
    for (goal, targets) in &goals {
        let goal = *goal;
        // The demand: the goal's presence (a constant 1 for a mandatory goal).
        let demand = if encoder.transitions.get_prez(goal, ctx) == Lit::TRUE {
            LinExpr::from_constant(1)
        } else {
            ColTag::PresenceTransition(goal, None).into_linexpr()
        };
        // The terms of the balance rows: ((ground state variable, value), column, coefficient).
        let mut balance_terms = vec![];

        for state_var in targets.iter().map(|&(_, state_var, _)| state_var).unique() {
            let entries = by_state_var.get(&state_var).copied().unwrap_or_default();
            for arc_entries in entries.chunk_by(|a, b| a.0 == b.0) {
                let g = arc_entries[0].0;
                let Some(post_val) = g.post_op_assignment else {
                    continue;
                };
                if g.pre_val_assignment == Some(post_val) {
                    continue; // (doesn't change the value)
                }
                let flow = ColTag::GoalCommodityFlow(goal, g);
                let presences = sum(arc_entries
                    .iter()
                    .map(|&(g, trans_id)| ColTag::PresenceTransition(trans_id, Some(g))));

                // Capacity: "An arc's flow is at most the total presence of the ground transitions corresponding to that arc"
                problem.push_row(flow.into_linexpr().leq(presences));
                num_cols += 1;
                num_rows += 1;

                balance_terms.push(((state_var, post_val), flow, 1));
                balance_terms.extend(g.pre_val_assignment.map(|pre_val| ((state_var, pre_val), flow, -1)));
            }
        }

        // With a single grounding, the goal's presence is the demand at its required value (see below).
        // Otherwise, a sink receives it.
        let single_target = if let [(_, state_var, value)] = targets[..] {
            Some((state_var, value))
        } else {
            // Sink: "A grounding's sink arc carries at most the grounding's presence, and the sink receives the goal's presence"
            let mut sink_inflow = vec![];
            for &(g, state_var, value) in targets {
                let sink = ColTag::GoalCommoditySink(goal, g);
                problem.push_row(sink.into_linexpr().leq(ColTag::PresenceTransition(goal, Some(g))));
                num_cols += 1;
                num_rows += 1;
                balance_terms.push(((state_var, value), sink, -1));
                sink_inflow.push(sink);
            }
            problem.push_row(sum(sink_inflow).eq(demand.clone()));
            num_rows += 1;
            None
        };

        // Balance: "The flow into a value minus the flow out of it (including into the sink) is 0,
        //           or the goal's presence at the required value of a single grounding"
        balance_terms.sort_unstable_by_key(|&(node, _, _)| node);
        if let Some(target) = single_target
            && !balance_terms.iter().any(|&(node, _, _)| node == target)
        {
            // (no arc reaches the required value: the goal can't be present)
            problem.push_row(LinExpr::zero().eq(demand.clone()));
            num_rows += 1;
        }
        for node_terms in balance_terms.chunk_by(|a, b| a.0 == b.0) {
            let row = node_terms
                .iter()
                .map(|&(_, col, coef)| (col, coef))
                .collect::<LinExpr>();
            let rhs = if Some(node_terms[0].0) == single_target {
                demand.clone()
            } else {
                LinExpr::zero()
            };
            problem.push_row(row.eq(rhs));
            num_rows += 1;
        }
    }

    lprelax_log!(
        "Goal commodities: {} goals routed ({num_cols} ground columns, {num_rows} ground rows), skipped: {:?}",
        goals.len(),
        skipped,
    );
}
