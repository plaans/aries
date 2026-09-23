//! Simulated preference enforcement for automated experimental evaluation.
//!
//! Three strategies:
//!   - **Greedy**: descending utility order; runs [`run_one_by_one`] once.
//!   - **AllAtOnce**: selects all violated preferences at once, prunes
//!     until feasible ([`run_prune`]), then attempts to re-add dropped
//!     preferences one by one via [`run_one_by_one`] (LIFO order).
//!   - **Blind**: like Greedy but the simulated user receives NO
//!     explanation: on conflict it drops the lowest-utility preference of
//!     its whole selection (not of the MUS) and retries. Baseline that
//!     isolates the contribution of the MUS explanations.
//!
//! Conflict resolution (Greedy/AllAtOnce): analyze MUSes to find which
//! preferences are in conflict, then drop the one with the lowest utility.
//!
//! User models (`--user-model`, orthogonal to the strategy):
//!   - **original**: asks only for the preferences violated by the planner's
//!     optimum (June 2026 behaviour). Never reacts to sacrifices.
//!   - **protective**: asks for ALL preferences in utility order, including the
//!     ones already satisfied, pinning them so that a later solve cannot
//!     sacrifice a valuable goal silently. Each protection costs a step.
//!   - **reactive**: asks only for violated preferences, but whenever a solve
//!     sacrifices a preference that was satisfied before, re-requests it
//!     immediately (front of the queue). A re-request that fails is given up.
//!
//! Hidden utilities: with `--user-utilities <problem.pddl>` the simulated
//! user orders and drops preferences with the weights of THAT problem's
//! metric, while the planner keeps optimizing its own metric. The output
//! then reports the user's utility of the planner's optimum and of the
//! final plan.

use std::collections::{BTreeMap, BTreeSet};

use aries::prelude::*;
use aries_plan_engine::encode::{encoding::Encoding, tags::Tag};

use planx::{Expr, ExprId, Fun, Metric, Model};
use timelines::{IntTerm, Sched, explain::ExplainableSolver};

use crate::explain_preferences::{
    Outcome, PreferenceEntry,
    build_preference_entries, collect_mus_mcs, compute_plan_cost,
    compute_still_violated, display_conflicts,
    print_preference_list, run_phase1, run_phase2,
};

// =====================================================================
// Strategy enum (CLI flag)
// =====================================================================

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub(crate) enum Strategy {
    /// Enforce violated preferences by descending utility; drop cheapest on conflict.
    Greedy,
    /// Select all violated at once, trim until feasible.
    AllAtOnce,
    /// Greedy without explanations: on conflict drop the cheapest of the whole selection.
    Blind,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strategy::Greedy => write!(f, "greedy"),
            Strategy::AllAtOnce => write!(f, "all-at-once"),
            Strategy::Blind => write!(f, "blind"),
        }
    }
}

/// How the simulated user chooses what to ask for (orthogonal to `Strategy`).
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserModel {
    /// Only the preferences violated by the planner's optimum; ignores sacrifices.
    Original,
    /// All preferences in utility order, including satisfied ones (pins what it values).
    Protective,
    /// Violated preferences, plus re-requests any preference sacrificed by a solve.
    Reactive,
}

impl std::fmt::Display for UserModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserModel::Original => write!(f, "original"),
            UserModel::Protective => write!(f, "protective"),
            UserModel::Reactive => write!(f, "reactive"),
        }
    }
}

// =====================================================================
// Metrics — same format as interactive mode for direct comparison.
// Printed via Drop on any exit path.
// =====================================================================

struct SimulationMetrics { steps: usize, outcome: Outcome }

impl Drop for SimulationMetrics {
    fn drop(&mut self) {
        if self.steps > 0 {
            println!("\nInteraction steps: {} (outcome: {})", self.steps, self.outcome);
        }
    }
}

// =====================================================================
// Utility extraction — walk the PDDL metric expression tree
// (minimize(sum(weight_i * is-violated(pref_i)))) to get a map of
// preference_name -> weight (higher = more important to satisfy).
// =====================================================================

pub(crate) fn extract_preference_weights(model: &Model) -> BTreeMap<String, i64> {
    let mut weights = BTreeMap::new();
    let metric_expr_id = match model.metric {
        Some(Metric::Minimize(id)) => id,
        _ => return weights,
    };
    collect_weights(model, metric_expr_id, &mut weights);
    weights
}

fn collect_weights(model: &Model, expr_id: ExprId, weights: &mut BTreeMap<String, i64>) {
    match model.env.node(expr_id).expr() {
        Expr::App(Fun::Plus, children) => {
            for &child in children.iter() {
                collect_weights(model, child, weights);
            }
        }
        Expr::App(Fun::Mul, children) if children.len() == 2 => {
            let node_a = model.env.node(children[0]);
            let node_b = model.env.node(children[1]);
            match (node_a.expr(), node_b.expr()) {
                (Expr::ViolationCount(name), Expr::Real(w))
                | (Expr::Real(w), Expr::ViolationCount(name)) => {
                    weights.insert(name.as_str().to_string(), *w.numer());
                }
                _ => {}
            }
        }
        Expr::ViolationCount(name) => {
            weights.insert(name.as_str().to_string(), 1);
        }
        _ => {}
    }
}

// =====================================================================
// Conflict resolution — from MUSes, pick the preference with the
// lowest utility. Tie-breaking: alphabetical name.
// =====================================================================

fn pick_cheapest_in_conflict(
    muses: &[BTreeSet<Tag>],
    selected_names: &BTreeSet<String>,
    weights: &BTreeMap<String, i64>,
) -> Option<String> {
    let in_conflict: BTreeSet<&str> = muses.iter()
        .flat_map(|mus| mus.iter())
        .filter_map(|tag| match tag {
            Tag::EnforcePreference(name) if selected_names.contains(name.as_str()) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    in_conflict.iter()
        .min_by(|&&a, &&b| {
            let wa = weights.get(a).copied().unwrap_or(0);
            let wb = weights.get(b).copied().unwrap_or(0);
            wa.cmp(&wb).then_with(|| a.cmp(b))
        })
        .map(|&name| name.to_string())
}

// =====================================================================
// Timing of solver calls (feasibility probes and MUS/MCS analyses).
// Printed per call as [SIM][label] probe_ms / analysis_ms and summarized
// at the end, so that the cost of the explanations can be measured.
// =====================================================================

#[derive(Default)]
pub(crate) struct SimTimers {
    probes: usize,
    probe_ms: u128,
    analyses: usize,
    analysis_ms: u128,
}

impl SimTimers {
    fn report(&self) {
        println!("[SIM] Probes: {} ({} ms total), MUS/MCS analyses: {} ({} ms total)",
            self.probes, self.probe_ms, self.analyses, self.analysis_ms);
    }
}

fn timed<T>(count: &mut usize, acc_ms: &mut u128, path_label: &str, kind: &str, f: impl FnOnce() -> T) -> T {
    let start = std::time::Instant::now();
    let result = f();
    let ms = start.elapsed().as_millis();
    *count += 1;
    *acc_ms += ms;
    println!("[SIM][{}] {}_ms: {}", path_label, kind, ms);
    result
}

/// User utility (sum of `weights`) of the preferences satisfied in a solution.
fn user_utility_of(entries: &[PreferenceEntry], satisfied: &BTreeSet<String>, weights: &BTreeMap<String, i64>) -> i64 {
    entries.iter()
        .filter(|e| satisfied.contains(&e.name))
        .map(|e| weights.get(&e.name).copied().unwrap_or(0))
        .sum()
}

fn satisfied_in(entries: &[PreferenceEntry], sol: &Solution) -> BTreeSet<String> {
    entries.iter()
        .filter(|e| e.lits.iter().all(|lit| sol.entails(*lit)))
        .map(|e| e.name.clone())
        .collect()
}

// =====================================================================
// One-by-one enforcement.
//
// Single pass: try each preference in `ordering`, solve on success,
// drop on conflict.  Used by Greedy (from empty set) and by the
// AllAtOnce re-add phase (from the surviving set after pruning).
// =====================================================================

struct PathResult {
    steps: usize,
    outcome: Outcome,
    enforced: Vec<usize>,
    final_obj_val: Option<IntCst>,
    /// Preferences satisfied by the last solution found (for user-utility reporting).
    final_satisfied: Option<BTreeSet<String>>,
}

fn run_one_by_one(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    model: &Model,
    sched: &Sched,
    entries: &[PreferenceEntry],
    plan_cost_obj: Option<IntTerm>,
    optimal_obj_val: IntCst,
    optimal_plan_cost: Option<IntCst>,
    stop_threshold: Option<IntCst>,
    initial_set: &[usize],
    initial_steps: usize,
    ordering: &[usize],
    path_label: &str,
    // Blind mode: no MUS analysis; on conflict drop the lowest-`weights`
    // preference of the whole selection and retry the same request.
    blind: bool,
    weights: &BTreeMap<String, i64>,
    timers: &mut SimTimers,
    user_model: UserModel,
    // Preferences satisfied before this loop started (planner's optimum, or the
    // last known solution); used by the Reactive model to detect sacrifices.
    baseline_satisfied: &BTreeSet<String>,
) -> PathResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = initial_set.to_vec();
    let mut steps: usize = initial_steps;
    let mut last_obj_val: Option<IntCst> = None;
    let mut last_satisfied: Option<BTreeSet<String>> = None;
    let mut prev_satisfied: BTreeSet<String> = baseline_satisfied.clone();

    // Pending requests. The Reactive model pushes sacrificed preferences to the
    // front; a preference that was requested and dropped is never asked again.
    let mut pending: std::collections::VecDeque<usize> = ordering.iter().copied().collect();
    let mut given_up: BTreeSet<usize> = BTreeSet::new();

    while let Some(next_idx) = pending.pop_front() {
        if selected_indices.contains(&next_idx) || given_up.contains(&next_idx) {
            continue;
        }
        steps += 1;
        selected_indices.push(next_idx);
        println!("\n[SIM][{}] Step {}: enforce {}", path_label, steps, entries[next_idx].name);

        let mut selected_pref_lits: Vec<Lit> = selected_indices.iter()
            .flat_map(|&i| entries[i].lits.iter().copied()).collect();
        let selected_names: BTreeSet<String> = selected_indices.iter()
            .map(|&i| entries[i].name.clone()).collect();

        if !blind {
            // Greedy / re-add phase: the conflict analysis is shown and the
            // just-added preference (cheapest of the selection by construction)
            // is dropped.
            let feasible = timed(&mut timers.probes, &mut timers.probe_ms, path_label, "probe",
                || crate::explain_preferences::probe_feasibility(solver, obj, &selected_pref_lits));
            if !feasible {
                println!("\n[SIM][{}] Infeasible, analyzing conflicts...", path_label);
                let analysis = timed(&mut timers.analyses, &mut timers.analysis_ms, path_label, "analysis",
                    || collect_mus_mcs(solver, entries, &selected_indices, &selected_names));
                display_conflicts(&analysis.muses, &selected_names, model);
                steps += 1;
                println!("[SIM][{}] Drop: {}", path_label, entries[next_idx].name);
                selected_indices.pop();
                given_up.insert(next_idx);
                continue;
            }
        } else {
            // Blind: no explanation. While infeasible, drop the cheapest of the
            // WHOLE selection (ties: name) and retry; if the dropped one is the
            // request itself, give up on it.
            let mut gave_up = false;
            loop {
                let feasible = timed(&mut timers.probes, &mut timers.probe_ms, path_label, "probe",
                    || crate::explain_preferences::probe_feasibility(solver, obj, &selected_pref_lits));
                if feasible { break; }
                println!("\n[SIM][{}] Infeasible (no explanation available)", path_label);
                let cheapest_pos = selected_indices.iter().enumerate()
                    .min_by(|x, y| {
                        let (a, b) = (*x.1, *y.1);
                        let wa = weights.get(&entries[a].name).copied().unwrap_or(0);
                        let wb = weights.get(&entries[b].name).copied().unwrap_or(0);
                        wa.cmp(&wb).then_with(|| entries[a].name.cmp(&entries[b].name))
                    })
                    .map(|(pos, _)| pos)
                    .expect("selection cannot be empty here");
                let dropped = selected_indices.remove(cheapest_pos);
                given_up.insert(dropped);
                steps += 1;
                println!("[SIM][{}] Drop cheapest of selection: {}", path_label, entries[dropped].name);
                if dropped == next_idx || selected_indices.is_empty() {
                    gave_up = true;
                    break;
                }
                // retry the same request with the reduced selection
                steps += 1;
                println!("[SIM][{}] Step {}: retry enforce {}", path_label, steps, entries[next_idx].name);
                selected_pref_lits = selected_indices.iter()
                    .flat_map(|&i| entries[i].lits.iter().copied()).collect();
            }
            if gave_up { continue; }
        }

        let mut latest_solution = run_phase1(
            solver, encoding, sched, entries, &selected_indices,
            &selected_pref_lits, obj, optimal_obj_val, optimal_plan_cost,
        );
        if latest_solution.is_none() {
            if let Some(cost_obj) = plan_cost_obj {
                latest_solution = run_phase2(
                    solver, encoding, sched, entries, &selected_indices,
                    &selected_pref_lits, obj, optimal_obj_val, optimal_plan_cost, cost_obj,
                );
            }
        }

        if let Some(ref sol) = latest_solution {
            last_obj_val = Some(sol.eval(obj).unwrap());
            let now_satisfied = satisfied_in(entries, sol);
            if user_model == UserModel::Reactive {
                // Preferences the user had (satisfied before) and lost with this
                // solve, not enforced, not pending and not given up: ask for them
                // back, most valuable first.
                let sacrificed: Vec<usize> = entries.iter().enumerate()
                    .filter(|(i, e)| prev_satisfied.contains(&e.name)
                        && !now_satisfied.contains(&e.name)
                        && !selected_indices.contains(i)
                        && !given_up.contains(i)
                        && !pending.contains(i))
                    .map(|(i, _)| i)
                    .collect();
                if !sacrificed.is_empty() {
                    let sacrificed = build_greedy_ordering(&sacrificed, entries, weights);
                    println!("[SIM][{}] Sacrificed by the solver, re-requesting: [{}]", path_label,
                        sacrificed.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", "));
                    for &i in sacrificed.iter().rev() {
                        pending.push_front(i);
                    }
                }
            }
            prev_satisfied = now_satisfied.clone();
            last_satisfied = Some(now_satisfied);
        }

        if let (Some(current), Some(threshold)) = (last_obj_val, stop_threshold) {
            if current <= threshold {
                println!("\n[SIM][{}] Stop threshold reached: objective {} ≤ threshold {}",
                    path_label, current, threshold);
                return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val, final_satisfied: last_satisfied };
            }
        }

        if let Some(ref sol) = latest_solution {
            let still_violated = compute_still_violated(entries, &selected_indices, sol);
            if still_violated.is_empty() {
                println!("\n[SIM][{}] All preferences are now satisfied.", path_label);
                return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val, final_satisfied: last_satisfied };
            }
        }
    }

    println!("\n[SIM][{}] No more preferences to try.", path_label);
    PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val, final_satisfied: last_satisfied }
}

// =====================================================================
// AllAtOnce — select all violated, prune until feasible.
// =====================================================================

struct PruneResult {
    selected: Vec<usize>,
    steps: usize,
    final_obj_val: Option<IntCst>,
    final_satisfied: Option<BTreeSet<String>>,
}

fn run_prune(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    model: &Model,
    sched: &Sched,
    entries: &[PreferenceEntry],
    weights: &BTreeMap<String, i64>,
    plan_cost_obj: Option<IntTerm>,
    optimal_obj_val: IntCst,
    optimal_plan_cost: Option<IntCst>,
    violated_sorted: &[usize],
    path_label: &str,
    timers: &mut SimTimers,
    user_model: UserModel,
    baseline_satisfied: &BTreeSet<String>,
) -> PruneResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = violated_sorted.to_vec();
    let mut steps: usize = 1;
    // Actual drop order of the prune, so that the re-add phase can be LIFO
    // (last dropped = most valuable among the dropped, retried first).
    let mut drop_order: Vec<usize> = Vec::new();

    let selected_str = selected_indices.iter()
        .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
    println!("\n[SIM][{}] Step 1: enforce all [{}]", path_label, selected_str);

    // Trim until feasible
    loop {
        if selected_indices.is_empty() {
            println!("\n[SIM][{}] All preferences were dropped.", path_label);
            break;
        }

        let selected_pref_lits: Vec<Lit> = selected_indices.iter()
            .flat_map(|&i| entries[i].lits.iter().copied()).collect();
        let selected_names: BTreeSet<String> = selected_indices.iter()
            .map(|&i| entries[i].name.clone()).collect();

        let feasible = timed(&mut timers.probes, &mut timers.probe_ms, path_label, "probe",
            || crate::explain_preferences::probe_feasibility(solver, obj, &selected_pref_lits));
        if feasible {
            break;
        }

        println!("\n[SIM][{}] Selection infeasible, analyzing conflicts...", path_label);
        let analysis = timed(&mut timers.analyses, &mut timers.analysis_ms, path_label, "analysis",
            || collect_mus_mcs(solver, entries, &selected_indices, &selected_names));
        display_conflicts(&analysis.muses, &selected_names, model);

        steps += 1;
        match pick_cheapest_in_conflict(&analysis.muses, &selected_names, weights) {
            Some(name_to_drop) => {
                println!("[SIM][{}] Drop cheapest in conflict: {}", path_label, name_to_drop);
                let dropped_idx = selected_indices.iter()
                    .position(|&i| entries[i].name == name_to_drop).unwrap();
                drop_order.push(selected_indices.remove(dropped_idx));
            }
            None => {
                break;
            }
        }
    }

    // Re-add phase: reuse the greedy one-by-one loop to try recovering
    // dropped preferences in LIFO order of the ACTUAL drops (the prune drops
    // cheapest-in-conflict first, so the last dropped are the most valuable
    // and are retried first).
    // Each successful re-add is followed by a full solve (Phase 1 / Phase 2),
    // exactly like the Greedy strategy.
    // A single pass suffices: each re-add only grows the selected set,
    // so a preference that fails feasibility cannot succeed later.
    let readd_ordering: Vec<usize> = drop_order.iter().copied().rev().collect();

    let mut final_obj_val: Option<IntCst> = None;
    let mut final_satisfied: Option<BTreeSet<String>> = None;

    if !readd_ordering.is_empty() && !selected_indices.is_empty() {
        let readd_str = readd_ordering.iter()
            .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
        println!("\n[SIM][{}] Re-add phase: attempting to recover [{}]", path_label, readd_str);

        let readd_result = run_one_by_one(
            solver, encoding, model, sched, entries,
            plan_cost_obj, optimal_obj_val, optimal_plan_cost,
            None, &selected_indices, steps, &readd_ordering, path_label,
            false, weights, timers, user_model, baseline_satisfied,
        );
        steps = readd_result.steps;
        selected_indices = readd_result.enforced;
        final_obj_val = readd_result.final_obj_val;
        final_satisfied = readd_result.final_satisfied;
    }

    // Solve the surviving set (only needed if no re-add ran or none succeeded).
    if final_obj_val.is_none() && !selected_indices.is_empty() {
        let selected_pref_lits: Vec<Lit> = selected_indices.iter()
            .flat_map(|&i| entries[i].lits.iter().copied()).collect();

        let mut latest_solution = run_phase1(
            solver, encoding, sched, entries, &selected_indices,
            &selected_pref_lits, obj, optimal_obj_val, optimal_plan_cost,
        );
        if latest_solution.is_none() {
            if let Some(cost_obj) = plan_cost_obj {
                latest_solution = run_phase2(
                    solver, encoding, sched, entries, &selected_indices,
                    &selected_pref_lits, obj, optimal_obj_val, optimal_plan_cost, cost_obj,
                );
            }
        }
        if let Some(ref sol) = latest_solution {
            final_obj_val = Some(sol.eval(obj).unwrap());
            final_satisfied = Some(satisfied_in(entries, sol));
        }

        // Reactive model: the final solve may have sacrificed preferences the
        // user had; ask for them back through the one-by-one loop.
        if user_model == UserModel::Reactive {
            if let Some(ref sat) = final_satisfied {
                let sacrificed: Vec<usize> = entries.iter().enumerate()
                    .filter(|(i, e)| baseline_satisfied.contains(&e.name)
                        && !sat.contains(&e.name)
                        && !selected_indices.contains(i))
                    .map(|(i, _)| i)
                    .collect();
                if !sacrificed.is_empty() {
                    let sacrificed = build_greedy_ordering(&sacrificed, entries, weights);
                    println!("\n[SIM][{}] Sacrificed by the solver, re-requesting: [{}]", path_label,
                        sacrificed.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", "));
                    let react = run_one_by_one(
                        solver, encoding, model, sched, entries,
                        plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                        None, &selected_indices, steps, &sacrificed, path_label,
                        false, weights, timers, user_model, sat,
                    );
                    steps = react.steps;
                    selected_indices = react.enforced;
                    if react.final_obj_val.is_some() {
                        final_obj_val = react.final_obj_val;
                        final_satisfied = react.final_satisfied;
                    }
                }
            }
        }
    }

    let surviving_str = selected_indices.iter()
        .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
    println!("\n[SIM][{}] Prune done: {} steps, surviving: [{}]",
        path_label, steps, surviving_str);

    PruneResult { selected: selected_indices, steps, final_obj_val, final_satisfied }
}

// =====================================================================
// Greedy ordering — descending utility.
// =====================================================================

fn build_greedy_ordering(violated_indices: &[usize], entries: &[PreferenceEntry], weights: &BTreeMap<String, i64>) -> Vec<usize> {
    let mut sorted: Vec<usize> = violated_indices.to_vec();
    sorted.sort_by(|&a, &b| {
        let wa = weights.get(&entries[a].name).copied().unwrap_or(0);
        let wb = weights.get(&entries[b].name).copied().unwrap_or(0);
        wb.cmp(&wa).then_with(|| entries[a].name.cmp(&entries[b].name))
    });
    sorted
}

// =====================================================================
// Main entry point — called from optimize_plan when --simulate is set.
// =====================================================================

pub(crate) fn simulate_preference_enforcement(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    model: &Model,
    sched: &Sched,
    optimal_solution: &Solution,
    phase_assumptions: &[Lit],
    plan_cost_obj: Option<IntTerm>,
    strategy: Strategy,
    cli_stop_threshold: Option<i64>,
    // Utilities of the simulated user (from --user-utilities). If None, the
    // user shares the planner's metric weights (original behaviour).
    user_weights: Option<&BTreeMap<String, i64>>,
    user_model: UserModel,
) {
    if encoding.preferences.is_empty() { return; }

    println!("\nSimulation strategy: {}", strategy);
    println!("[SIM] User model: {}", user_model);

    // Baseline values from the optimal (no-preferences) solution
    let obj = encoding.objectives[0];
    let optimal_obj_val = optimal_solution.eval(obj).unwrap();
    let optimal_plan_cost = compute_plan_cost(sched, optimal_solution);
    let entries = build_preference_entries(encoding, model, optimal_solution);
    let planner_weights = extract_preference_weights(model);
    // All decisions of the simulated user (ordering, drops) use `weights`.
    let weights: BTreeMap<String, i64> = match user_weights {
        Some(w) => w.clone(),
        None => planner_weights.clone(),
    };
    let hidden = user_weights.is_some();
    let mut timers = SimTimers::default();

    println!("\n===== Preference satisfaction (simulated mode) =====\n");
    println!("Optimal objective value: {}", optimal_obj_val);
    if let Some(cost) = optimal_plan_cost { println!("Optimal plan cost: {}", cost); }
    println!();

    let violated_indices: Vec<usize> = entries.iter().enumerate()
        .filter(|(_, e)| !e.is_satisfied).map(|(i, _)| i).collect();

    print_preference_list(&entries);

    if violated_indices.is_empty() {
        println!("\nAll preferences are satisfied.");
        return;
    }

    println!("\nPreference weights: {}",
        entries.iter().map(|e| format!("{}={}", e.name, weights.get(&e.name).copied().unwrap_or(0)))
            .collect::<Vec<_>>().join(", "));
    if hidden {
        println!("Planner weights: {}",
            entries.iter().map(|e| format!("{}={}", e.name, planner_weights.get(&e.name).copied().unwrap_or(0)))
                .collect::<Vec<_>>().join(", "));
        let missing: Vec<&str> = entries.iter()
            .filter(|e| !weights.contains_key(&e.name)).map(|e| e.name.as_str()).collect();
        if !missing.is_empty() {
            println!("[SIM] WARNING: preferences without user utility (weight 0): {}", missing.join(", "));
        }
    }
    // User utility of the planner's optimum (baseline for "recovered utility").
    let total_user_utility: i64 = entries.iter().map(|e| weights.get(&e.name).copied().unwrap_or(0)).sum();
    let baseline_satisfied: BTreeSet<String> = entries.iter()
        .filter(|e| e.is_satisfied).map(|e| e.name.clone()).collect();
    let baseline_user_utility = user_utility_of(&entries, &baseline_satisfied, &weights);
    let report_user_utility = |final_satisfied: &Option<BTreeSet<String>>| {
        let final_u = match final_satisfied {
            Some(sat) => user_utility_of(&entries, sat, &weights),
            None => baseline_user_utility,
        };
        println!("[SIM] User utility: total {}, planner optimum {}, final {}, gained {}",
            total_user_utility, baseline_user_utility, final_u, final_u - baseline_user_utility);
    };
    println!("\nViolated preferences: {}\n",
        violated_indices.iter().map(|&i| format!("{}", i + 1)).collect::<Vec<_>>().join(", "));

    solver.enforce_permanent(phase_assumptions);

    // Early stop threshold: only active if passed via --stop-threshold.
    let stop_threshold: Option<IntCst> = cli_stop_threshold.map(|v| {
        println!("[SIM] Stop threshold (from CLI): {}", v);
        v as IntCst
    });
    if stop_threshold.is_none() {
        println!("[SIM] Stop threshold: disabled (pass --stop-threshold to enable)");
    }

    // What the user asks for: only the violated preferences, or all of them
    // (Protective pins the satisfied ones too, in utility order).
    let candidate_indices: Vec<usize> = match user_model {
        UserModel::Protective => (0..entries.len()).collect(),
        UserModel::Original | UserModel::Reactive => violated_indices.clone(),
    };

    match strategy {
        Strategy::Greedy | Strategy::Blind => {
            // Enforce highest-utility candidates first.
            let blind = matches!(strategy, Strategy::Blind);
            let label = if blind { "blind" } else { "greedy" };
            let ordering = build_greedy_ordering(&candidate_indices, &entries, &weights);
            let result = run_one_by_one(
                &mut solver.clone(), encoding, model, sched, &entries,
                plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                stop_threshold, &[], 0, &ordering, label,
                blind, &weights, &mut timers, user_model, &baseline_satisfied,
            );
            let _metrics = SimulationMetrics { steps: result.steps, outcome: result.outcome };
            let enforced_str = result.enforced.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
            // "Greedy path enforced" is kept for Blind too so that the existing
            // log parsers (parse_console_logs.py) keep working.
            println!("\n[SIM] Greedy path enforced: [{}]", enforced_str);
            if let Some(final_val) = result.final_obj_val {
                println!("[SIM] Optimal objective: {}, final objective: {}, distance to optimal: {}",
                    optimal_obj_val, final_val, final_val - optimal_obj_val);
            }
            report_user_utility(&result.final_satisfied);
            timers.report();
        }
        Strategy::AllAtOnce => {
            let ordering = build_greedy_ordering(&candidate_indices, &entries, &weights);

            let prune = run_prune(
                &mut solver.clone(), encoding, model, sched, &entries, &weights,
                plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                &ordering, "all-at-once", &mut timers, user_model, &baseline_satisfied,
            );

            let enforced_str = prune.selected.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
            println!("\n[SIM] AllAtOnce enforced: [{}]", enforced_str);
            println!("[SIM] Total steps: {}", prune.steps);
            if let Some(final_val) = prune.final_obj_val {
                println!("[SIM] Optimal objective: {}, final objective: {}, distance to optimal: {}",
                    optimal_obj_val, final_val, final_val - optimal_obj_val);
            }
            report_user_utility(&prune.final_satisfied);
            timers.report();
            let _metrics = SimulationMetrics { steps: prune.steps, outcome: Outcome::Accepted };
        }
    }
}
