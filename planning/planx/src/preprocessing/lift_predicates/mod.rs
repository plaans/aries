//! Replacement of groups of mutually exclusive predicates by multi-valued state functions.
//!
//! # Principle
//!
//! A *group* is a set of predicates whose parameters are split into a *key* and (optionally) a *value*.
//! For instance, the group `{at(?b, ?r), carry(?b, ?g)}` has key `?b` and values `?r` and `?g`.
//! If, for any key, at most one atom of the group is true at any time, the group can be represented by a
//! state function `sv(key) -> value` where `sv(κ) = v` iff `p(κ, v)` is the true atom of key `κ`,
//! and `sv(κ)` is undefined (`∅`) if no atom of key `κ` is true.
//! Predicates without a value parameter are represented by a dedicated helper object (e.g. `{full(?s), empty(?s)}`
//! becomes `sv(?s) ∈ {_help-obj-full, _help-obj-empty}`).
//!
//! # Conditions
//!
//! A group is lifted if all the following conditions hold:
//!
//! - **(G1) usage**: its predicates only appear as *positive* atoms of conditions, goals or constraints
//!   (not under a negation, in a preference, in an effect condition, ...). Such an atom `p(κ, v)` is then
//!   equivalent to `(= sv(κ) v)`, without relying on how the solver evaluates an undefined state variable.
//! - **(G2) initial state**: it contains at most one atom of the group per key, as plain facts at the origin.
//! - **(G3) values**: the values of distinct predicates of a group are distinct (helper objects, or values with
//!   provably disjoint sets of possible objects), and have a common supertype.
//! - **(G4) actions**: in each action, the effects on the group are unconditional and unquantified, and, grouped by
//!   (syntactic) key, follow the pattern **P**:
//!   - (P1) at most one atom is added, possibly several times but all at the same timepoint `t_add`;
//!   - (P2) all deletes happen no later than `t_add` (if any), at or after the first one `t₀`.
//!     If there is no add, all deletes happen at `t₀`;
//!   - (P3) one of the deletes at `t₀` is *required*: the action has a positive condition on this very atom
//!     that holds just before `t₀`, i.e. over an interval containing `t₀` or an open interval ending at `t₀`
//!     (e.g. `(over all (p))` with `(at end (not (p)))`: `p` holds until `t₀` and could only be changed by
//!     another effect at `t₀`, which would conflict with the delete);
//!   - (P3') alternatively, for groups without value parameters: the action adds an atom (or not) and deletes all other
//!     atoms of the key at the same time `t₀ = t_add`. This is only allowed if no action of the group *holds* a key
//!     (i.e. has `t₀ < t_add`);
//!   - (P4) if two distinct keys of the action may denote the same objects (e.g. `?x` and `?y` of overlapping types,
//!     without a `(not (= ?x ?y))` condition), the atoms required in (P3) for these keys are distinct atoms and
//!     are required at a common instant.
//!
//! Orderings of the timepoints of an action are those provable from the bounds of its duration
//! (see [`ActionTiming`](crate::preprocessing::analysis::ActionTiming)): no assumption is made on the sign of durations.
//!
//! The update of a key `κ` is then rewritten as `sv(κ) := ∅` at `t₀` (omitted if `t₀ = t_add`)
//! and `sv(κ) := value` at `t_add`.
//!
//! # Why this is correct
//!
//! Recall that conditions at a time `t` are evaluated before the effects at `t` and that two distinct actions
//! cannot have effects on the same state variable at the same time.
//! For a key `κ` and an instant `t`, let `N(κ, t)` be the number of true atoms of key `κ` plus the number of
//! "holders" of `κ`: actions that passed their `t₀` for `κ` but not yet their `t_add`.
//! We show by induction on the plan's happenings that `N(κ, t) ≤ 1` (thus at most one atom per key is true):
//! - initially, `N ≤ 1` by (G2).
//! - an action whose `t₀` is at `t` requires (P3) a true atom `r` of key `κ` at `t`: hence there is no holder and
//!   `r` is the only true atom. The action deletes `r` and either adds an atom (`t_add = t₀`, `N` unchanged),
//!   becomes the holder (`N` unchanged), or does nothing else (`N` decreases).
//! - an action whose `t_add` is at `t > t₀` is the holder and its add only restores `N`. Its other deletes
//!   (between `t₀` and `t_add`) are no-ops as no atom of `κ` is true while it holds `κ`.
//! - an action relying on (P3') at `t` sets the atoms of `κ` to exactly `{a}` (or `∅`), whatever the previous state.
//!   As there are no holders in this case, `N` is the number of true atoms, and remains `≤ 1`.
//! - two actions may not act on `κ` at the same instant: both at their `t₀` would require (and delete) the same
//!   atom `r` (forbidden); one at its `t₀` requires a true atom while the other holds `κ` (impossible);
//!   both at their `t_add` would be two holders (`N ≥ 2`); an action relying on (P3') modifies all atoms of `κ` and
//!   thus conflicts with any other action modifying `κ` at the same time.
//! - within an action, two keys that are bound to the same object at runtime are covered by (P4): the action would
//!   require two distinct atoms of the same key at the same instant, which is impossible, in both models.
//! - if, for an action instance, timepoints that are not provably equal coincide (e.g. zero duration),
//!   the happenings merge, which the above argument tolerates (the delete of `r` and the add are then simultaneous).
//!
//! With at most one atom per key, the rewritten effects produce exactly the state function value `α(state)` at every
//! instant, and positive conditions are equivalent (G1, G3), so any plan is valid in the original model iff it is valid
//! in the rewritten one.
//!
//! # Search
//!
//! Groups are found by refinement, in the spirit of the invariant synthesis of Helmert (2009, "Concise
//! finite-domain representations for PDDL planning tasks"): starting from single predicates (with each choice of value
//! parameter), a candidate whose action has an add effect not balanced by a required delete (P3) is extended with the
//! predicate of a delete required by this action (or, for (P3'), of a simultaneous delete). Candidates are bounded in size, which bounds the search to a
//! polynomial number of candidates, each checked in time linear in the size of the model.

mod groups;
mod rewrite;

use std::cmp::Reverse;
use std::collections::HashSet;

use itertools::Itertools;

use crate::{Model, Res};
use groups::{Analyzer, Group};
use rewrite::LiftedGroup;

/// Substitute groups of predicates by state functions where applicable (see the module documentation).
///
/// For instance the predicate `(at agent location) -> boolean` can usually be
/// transformed into the state function `(at agent) -> location`.
pub fn lift_predicates_to_state_functions(model: &mut Model) -> Res<()> {
    let analyzer = Analyzer::new(model);
    let mut candidates = analyzer.valid_groups();
    // nothing to gain if the predicate would remain boolean, or if it is never true
    candidates.retain(|g| (g.has_values() || g.members.len() > 1) && analyzer.may_be_true(g));
    // prefer larger groups and, among them, values on the last parameters
    candidates.sort_by_key(|g| {
        (
            Reverse(g.members.len()),
            Reverse(g.members.iter().map(|m| m.value).collect_vec()),
        )
    });

    // keep disjoint groups, each with its rewriting
    let mut used = HashSet::new();
    let mut selected = Vec::new();
    for group in candidates {
        if group.members.iter().all(|m| !used.contains(&m.fluent)) {
            used.extend(group.members.iter().map(|m| m.fluent)); // TODO: we should only add to used if selected right?
            let updates = analyzer.rewriting(&group).expect("valid group");
            selected.push((group, updates));
        }
    }
    let descriptions = selected
        .iter()
        .map(|(g, _)| (name(g, model), describe(g, model)))
        .collect_vec();
    // drop(analyzer);

    if !selected.is_empty() {
        println!("Lifting predicates to state functions:")
    }
    let mut lifted = Vec::with_capacity(selected.len());
    for ((group, updates), (name, description)) in selected.into_iter().zip(descriptions) {
        println!("  - {name:<40} [from: {description}]");
        lifted.push((LiftedGroup::declare(group, &name, &mut model.env)?, updates));
    }
    rewrite::rewrite(model, &lifted)
}

/// Name of the state function replacing the group.
fn name(group: &Group, model: &Model) -> String {
    group
        .members
        .iter()
        .map(|m| model.env.fluents.get(m.fluent).name())
        .join(":")
}

/// Human readable description of the group, e.g. `at(0) -> 1, carry(0) -> 1`.
fn describe(group: &Group, model: &Model) -> String {
    group
        .members
        .iter()
        .map(|m| {
            let value = m.value.map_or("Bool".to_string(), |v| v.to_string());
            format!(
                "{}({}) -> {value}",
                model.env.fluents.get(m.fluent).name(),
                m.key.iter().join(", ")
            )
        })
        .join(", ")
}

#[cfg(test)]
mod tests;
