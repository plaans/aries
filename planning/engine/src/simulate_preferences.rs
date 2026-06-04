//! Simulated preference enforcement for automated experimental evaluation.
//!
//! Two strategies:
//!   - **Greedy**: natural PDDL order; runs [`run_one_by_one`] once.
//!   - **AllAtOnce**: selects all violated preferences at once, prunes
//!     until feasible ([`run_prune`]), then re-adds dropped preferences
//!     via [`run_one_by_one`].
//!
//! The greedy behavior lives in conflict resolution (`pick_best_mcs`):
//! always drop the cheapest preference. This is shared by all strategies.
//! Dropped preferences are re-enqueued and retried later (the solver state
//! may have changed). Retries stop when a full pass makes no progress.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use aries::prelude::*;
use aries_plan_engine::encode::{encoding::Encoding, tags::Tag};

use planx::{Expr, ExprId, Fun, Metric, Model};
use timelines::{IntTerm, Sched, explain::ExplainableSolver};

use crate::explain_preferences::{
    Outcome, PreferenceEntry,
    build_preference_entries, collect_mus_mcs, compute_plan_cost,
    compute_still_violated, display_conflicts,
    print_preference_list, run_phase1, run_phase2,
    display_resolutions,
};

// =====================================================================
// Strategy enum (CLI flag)
// =====================================================================

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub(crate) enum Strategy {
    /// Enforce violated preferences by descending utility; drop cheapest on conflict.
    Greedy,
    /// Select all violated at once, trim until feasible, then re-add dropped.
    AllAtOnce,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strategy::Greedy => write!(f, "greedy"),
            Strategy::AllAtOnce => write!(f, "all-at-once"),
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

fn extract_preference_weights(model: &Model) -> BTreeMap<String, i64> {
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
// MCS scoring — on conflict, pick the resolution that drops the
// preference with the lowest utility.
// =====================================================================

/// Which selected preferences does this MCS drop?
fn mcs_dropped_prefs<'a>(mcs: &'a BTreeSet<Tag>, selected_names: &'a BTreeSet<String>) -> Vec<&'a str> {
    mcs.iter()
        .filter_map(|tag| match tag {
            Tag::EnforcePreference(name) if selected_names.contains(name.as_str()) => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

/// Pick the MCS that drops the cheapest preference (lowest utility).
/// Tie-breaking: fewest drops, then alphabetical name.
fn pick_best_mcs(mcses: &[BTreeSet<Tag>], selected_names: &BTreeSet<String>, weights: &BTreeMap<String, i64>) -> Option<usize> {
    mcses.iter().enumerate()
        .filter_map(|(i, mcs)| {
            let dropped = mcs_dropped_prefs(mcs, selected_names);
            if dropped.is_empty() { return None; }
            let min_weight = dropped.iter().map(|n| weights.get(*n).copied().unwrap_or(0)).min().unwrap();
            // Score: (lowest weight dropped, number of drops, name) — min wins
            Some((i, (min_weight, dropped.len(), dropped.iter().min().unwrap().to_string())))
        })
        .min_by_key(|(_, s)| s.clone())
        .map(|(i, _)| i)
}

// =====================================================================
// One-by-one enforcement loop.
//
// Uses a queue seeded with the initial ordering. On conflict, dropped
// preferences are pushed to the back for retry. Stops when the queue
// is empty or a full pass yields no successful enforcement.
// =====================================================================

struct PathResult {
    steps: usize,
    outcome: Outcome,
    enforced: Vec<usize>,
    /// Final objective value after enforcement (None if no solution found)
    final_obj_val: Option<IntCst>,
}

fn run_one_by_one(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    model: &Model,
    sched: &Sched,
    entries: &[PreferenceEntry],
    weights: &BTreeMap<String, i64>,
    plan_cost_obj: Option<IntTerm>,
    optimal_obj_val: IntCst,
    optimal_plan_cost: Option<IntCst>,
    stop_threshold: Option<IntCst>,
    initial_ordering: &[usize],
    initial_selected: &[usize],
    path_label: &str,
) -> PathResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = initial_selected.to_vec();
    let mut steps: usize = 0;
    let mut last_obj_val: Option<IntCst> = None;

    let mut queue: VecDeque<usize> = initial_ordering.iter().copied().collect();
    let mut consecutive_failures: usize = 0;

    while let Some(next_idx) = queue.pop_front() {
        // Already enforced from a previous iteration — skip without
        // affecting the failure counter (neither success nor failure).
        if selected_indices.contains(&next_idx) {
            continue;
        }

        steps += 1;
        selected_indices.push(next_idx);
        println!("\n[SIM][{}] Step {}: enforce {}", path_label, steps, entries[next_idx].name);

        // Resolve conflicts until feasible or empty
        loop {
            if selected_indices.is_empty() { break; }

            let selected_pref_lits: Vec<Lit> = selected_indices.iter()
                .flat_map(|&i| entries[i].lits.iter().copied()).collect();
            let selected_names: BTreeSet<String> = selected_indices.iter()
                .map(|&i| entries[i].name.clone()).collect();

            if crate::explain_preferences::probe_feasibility(solver, obj, &selected_pref_lits) {
                break;
            }

            println!("\n[SIM][{}] Selection infeasible, analyzing conflicts...", path_label);
            let mus_mcs = collect_mus_mcs(solver, entries, &selected_indices, &selected_names);
            display_conflicts(&mus_mcs.muses, &selected_names, model);
            display_resolutions(&mus_mcs.mcses, &selected_names, model);

            steps += 1;
            match pick_best_mcs(&mus_mcs.mcses, &selected_names, weights) {
                Some(best_idx) => {
                    let prefs_to_drop: BTreeSet<&str> = mcs_dropped_prefs(&mus_mcs.mcses[best_idx], &selected_names)
                        .into_iter().collect();
                    println!("[SIM][{}] Applying R{}: drop {}", path_label, best_idx + 1,
                        prefs_to_drop.iter().copied().collect::<Vec<_>>().join(", "));
                    let dropped_indices: Vec<usize> = selected_indices.iter()
                        .filter(|&&i| prefs_to_drop.contains(entries[i].name.as_str()))
                        .copied().collect();
                    selected_indices.retain(|&i| !prefs_to_drop.contains(entries[i].name.as_str()));
                    // Re-enqueue dropped preferences: solver state may change
                    // after future enforcements, making them feasible again.
                    for idx in dropped_indices {
                        queue.push_back(idx);
                    }
                    if selected_indices.is_empty() {
                        println!("\n[SIM][{}] All selected preferences were dropped.", path_label);
                    }
                }
                None => {
                    break;
                }
            }
        }

        // A "failure" is when next_idx itself was dropped — not when some
        // other preference was dropped as collateral (that counts as success
        // because next_idx survived and the solver state changed).
        if selected_indices.contains(&next_idx) {
            consecutive_failures = 0;
        } else {
            consecutive_failures += 1;
            // Full pass without progress: every queued preference was tried
            // and none survived. Solver state unchanged → retrying is futile.
            if consecutive_failures >= queue.len() && !queue.is_empty() {
                let remaining: Vec<&str> = queue.iter()
                    .filter(|&&i| !selected_indices.contains(&i))
                    .map(|&i| entries[i].name.as_str()).collect();
                println!("\n[SIM][{}] No progress — {} consecutive failures, stopping retries. Unenforced: [{}]",
                    path_label, consecutive_failures, remaining.join(", "));
                break;
            }
        }

        // Solve: Phase 1 (within cost bound), Phase 2 (relaxed) if needed
        if !selected_indices.is_empty() {
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

            // Track the objective value of the latest solution
            if let Some(ref sol) = latest_solution {
                last_obj_val = Some(sol.eval(obj).unwrap());
            }

            // Early stop: objective reached the user's acceptable level
            if let (Some(current), Some(threshold)) = (last_obj_val, stop_threshold) {
                if current <= threshold {
                    println!("\n[SIM][{}] Stop threshold reached: objective {} ≤ threshold {}",
                        path_label, current, threshold);
                    return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val };
                }
            }

            // All preferences satisfied — early exit
            if let Some(ref sol) = latest_solution {
                let still_violated = compute_still_violated(entries, &selected_indices, sol);
                if still_violated.is_empty() {
                    println!("\n[SIM][{}] All preferences are now satisfied.", path_label);
                    return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val };
                }
            }
        }
    }

    if queue.is_empty() {
        println!("\n[SIM][{}] No more preferences to try.", path_label);
    }
    PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val }
}

// =====================================================================
// AllAtOnce — Phase 1: select all violated, prune until feasible.
// Returns the surviving set and the dropped indices (in drop order).
// =====================================================================

struct PruneResult {
    selected: Vec<usize>,
    dropped: Vec<usize>,
    steps: usize,
    final_obj_val: Option<IntCst>,
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
    violated_indices: &[usize],
    path_label: &str,
) -> PruneResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = violated_indices.to_vec();
    let mut dropped: Vec<usize> = Vec::new();
    let mut steps: usize = 1;

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

        if crate::explain_preferences::probe_feasibility(solver, obj, &selected_pref_lits) {
            break;
        }

        println!("\n[SIM][{}] Selection infeasible, analyzing conflicts...", path_label);
        let mus_mcs = collect_mus_mcs(solver, entries, &selected_indices, &selected_names);
        display_conflicts(&mus_mcs.muses, &selected_names, model);
        display_resolutions(&mus_mcs.mcses, &selected_names, model);

        steps += 1;
        match pick_best_mcs(&mus_mcs.mcses, &selected_names, weights) {
            Some(best_idx) => {
                let prefs_to_drop: BTreeSet<&str> = mcs_dropped_prefs(&mus_mcs.mcses[best_idx], &selected_names)
                    .into_iter().collect();
                println!("[SIM][{}] Applying R{}: drop {}", path_label, best_idx + 1,
                    prefs_to_drop.iter().copied().collect::<Vec<_>>().join(", "));
                let dropped_indices: Vec<usize> = selected_indices.iter()
                    .filter(|&&i| prefs_to_drop.contains(entries[i].name.as_str()))
                    .copied().collect();
                selected_indices.retain(|&i| !prefs_to_drop.contains(entries[i].name.as_str()));
                dropped.extend(dropped_indices);
            }
            None => {
                break;
            }
        }
    }

    // Solve the surviving set
    let mut final_obj_val: Option<IntCst> = None;
    if !selected_indices.is_empty() {
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
        }
    }

    let surviving_str = selected_indices.iter()
        .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
    let dropped_str = dropped.iter()
        .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
    println!("\n[SIM][{}] Phase 1 done: {} steps, surviving: [{}], dropped: [{}]",
        path_label, steps, surviving_str, dropped_str);

    PruneResult { selected: selected_indices, dropped, steps, final_obj_val }
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
) {
    if encoding.preferences.is_empty() { return; }

    println!("\nSimulation strategy: {}", strategy);

    // Baseline values from the optimal (no-preferences) solution
    let obj = encoding.objectives[0];
    let optimal_obj_val = optimal_solution.eval(obj).unwrap();
    let optimal_plan_cost = compute_plan_cost(sched, optimal_solution);
    let entries = build_preference_entries(encoding, model, optimal_solution);
    let weights = extract_preference_weights(model);

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

    match strategy {
        Strategy::Greedy => {
            // Enforce highest-utility violated preferences first.
            let ordering = build_greedy_ordering(&violated_indices, &entries, &weights);
            let result = run_one_by_one(
                &mut solver.clone(), encoding, model, sched, &entries, &weights,
                plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                stop_threshold, &ordering, &[], "greedy",
            );
            let _metrics = SimulationMetrics { steps: result.steps, outcome: result.outcome };
            let enforced_str = result.enforced.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
            println!("\n[SIM] Greedy path enforced: [{}]", enforced_str);
            if let Some(final_val) = result.final_obj_val {
                println!("[SIM] Optimal objective: {}, final objective: {}, distance to optimal: {}",
                    optimal_obj_val, final_val, final_val - optimal_obj_val);
            }
        }
        Strategy::AllAtOnce => {
            let mut sim_solver = solver.clone();
            let ordering = build_greedy_ordering(&violated_indices, &entries, &weights);

            // Phase 1: select all (sorted by descending utility), prune until feasible.
            let prune = run_prune(
                &mut sim_solver, encoding, model, sched, &entries, &weights,
                plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                &ordering, "all-at-once",
            );

            let (total_steps, final_obj, enforced) = if prune.dropped.is_empty() {
                (prune.steps, prune.final_obj_val, prune.selected.clone())
            } else {
                // Phase 2: re-add dropped preferences (in drop order) via the queue loop.
                println!("\n[SIM] Phase 2: re-adding {} dropped preferences...", prune.dropped.len());
                let phase2 = run_one_by_one(
                    &mut sim_solver, encoding, model, sched, &entries, &weights,
                    plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                    stop_threshold, &prune.dropped, &prune.selected, "all-at-once-readd",
                );
                let total = prune.steps + phase2.steps;
                println!("[SIM] Phase 1: {} steps, Phase 2: {} steps", prune.steps, phase2.steps);
                (total, phase2.final_obj_val.or(prune.final_obj_val), phase2.enforced)
            };

            let enforced_str = enforced.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
            println!("\n[SIM] AllAtOnce enforced: [{}]", enforced_str);
            println!("[SIM] Total steps: {}", total_steps);
            if let Some(final_val) = final_obj {
                println!("[SIM] Optimal objective: {}, final objective: {}, distance to optimal: {}",
                    optimal_obj_val, final_val, final_val - optimal_obj_val);
            }
            let _metrics = SimulationMetrics { steps: total_steps, outcome: Outcome::Accepted };
        }
    }
}
