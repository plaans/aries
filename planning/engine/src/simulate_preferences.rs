//! Simulated preference enforcement for automated experimental evaluation.
//!
//! Two strategies:
//!   - **Greedy**: descending utility order; runs [`run_one_by_one`] once.
//!   - **AllAtOnce**: selects all violated preferences at once, prunes
//!     until feasible ([`run_prune`]), then attempts to re-add dropped
//!     preferences one by one via [`run_one_by_one`] (LIFO order).
//!
//! Conflict resolution: analyze MUSes to find which preferences are in
//! conflict, then drop the one with the lowest utility.

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
) -> PathResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = initial_set.to_vec();
    let mut steps: usize = initial_steps;
    let mut last_obj_val: Option<IntCst> = None;

    for &next_idx in ordering {
        steps += 1;
        selected_indices.push(next_idx);
        println!("\n[SIM][{}] Step {}: enforce {}", path_label, steps, entries[next_idx].name);

        let selected_pref_lits: Vec<Lit> = selected_indices.iter()
            .flat_map(|&i| entries[i].lits.iter().copied()).collect();
        let selected_names: BTreeSet<String> = selected_indices.iter()
            .map(|&i| entries[i].name.clone()).collect();

        if !crate::explain_preferences::probe_feasibility(solver, obj, &selected_pref_lits) {
            println!("\n[SIM][{}] Infeasible, analyzing conflicts...", path_label);
            let analysis = collect_mus_mcs(solver, entries, &selected_indices, &selected_names);
            display_conflicts(&analysis.muses, &selected_names, model);
            steps += 1;
            println!("[SIM][{}] Drop: {}", path_label, entries[next_idx].name);
            selected_indices.pop();
            continue;
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
        }

        if let (Some(current), Some(threshold)) = (last_obj_val, stop_threshold) {
            if current <= threshold {
                println!("\n[SIM][{}] Stop threshold reached: objective {} ≤ threshold {}",
                    path_label, current, threshold);
                return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val };
            }
        }

        if let Some(ref sol) = latest_solution {
            let still_violated = compute_still_violated(entries, &selected_indices, sol);
            if still_violated.is_empty() {
                println!("\n[SIM][{}] All preferences are now satisfied.", path_label);
                return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val };
            }
        }
    }

    println!("\n[SIM][{}] No more preferences to try.", path_label);
    PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices, final_obj_val: last_obj_val }
}

// =====================================================================
// AllAtOnce — select all violated, prune until feasible.
// =====================================================================

struct PruneResult {
    selected: Vec<usize>,
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
    violated_sorted: &[usize],
    path_label: &str,
) -> PruneResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = violated_sorted.to_vec();
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
        let analysis = collect_mus_mcs(solver, entries, &selected_indices, &selected_names);
        display_conflicts(&analysis.muses, &selected_names, model);

        steps += 1;
        match pick_cheapest_in_conflict(&analysis.muses, &selected_names, weights) {
            Some(name_to_drop) => {
                println!("[SIM][{}] Drop cheapest in conflict: {}", path_label, name_to_drop);
                let dropped_idx = selected_indices.iter()
                    .position(|&i| entries[i].name == name_to_drop).unwrap();
                selected_indices.remove(dropped_idx);
            }
            None => {
                break;
            }
        }
    }

    // Re-add phase: reuse the greedy one-by-one loop to try recovering
    // dropped preferences.  The prune drops cheapest first, so reversing
    // gives a LIFO order that retries highest-utility drops first.
    // Each successful re-add is followed by a full solve (Phase 1 / Phase 2),
    // exactly like the Greedy strategy.
    // A single pass suffices: each re-add only grows the selected set,
    // so a preference that fails feasibility cannot succeed later.
    let dropped: Vec<usize> = violated_sorted.iter()
        .copied()
        .filter(|idx| !selected_indices.contains(idx))
        .collect();
    let readd_ordering: Vec<usize> = dropped.iter().copied().rev().collect();

    let mut final_obj_val: Option<IntCst> = None;

    if !readd_ordering.is_empty() && !selected_indices.is_empty() {
        let readd_str = readd_ordering.iter()
            .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
        println!("\n[SIM][{}] Re-add phase: attempting to recover [{}]", path_label, readd_str);

        let readd_result = run_one_by_one(
            solver, encoding, model, sched, entries,
            plan_cost_obj, optimal_obj_val, optimal_plan_cost,
            None, &selected_indices, steps, &readd_ordering, path_label,
        );
        steps = readd_result.steps;
        selected_indices = readd_result.enforced;
        final_obj_val = readd_result.final_obj_val;
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
        }
    }

    let surviving_str = selected_indices.iter()
        .map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
    println!("\n[SIM][{}] Prune done: {} steps, surviving: [{}]",
        path_label, steps, surviving_str);

    PruneResult { selected: selected_indices, steps, final_obj_val }
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
                &mut solver.clone(), encoding, model, sched, &entries,
                plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                stop_threshold, &[], 0, &ordering, "greedy",
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
            let ordering = build_greedy_ordering(&violated_indices, &entries, &weights);

            let prune = run_prune(
                &mut solver.clone(), encoding, model, sched, &entries, &weights,
                plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                &ordering, "all-at-once",
            );

            let enforced_str = prune.selected.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", ");
            println!("\n[SIM] AllAtOnce enforced: [{}]", enforced_str);
            println!("[SIM] Total steps: {}", prune.steps);
            if let Some(final_val) = prune.final_obj_val {
                println!("[SIM] Optimal objective: {}, final objective: {}, distance to optimal: {}",
                    optimal_obj_val, final_val, final_val - optimal_obj_val);
            }
            let _metrics = SimulationMetrics { steps: prune.steps, outcome: Outcome::Accepted };
        }
    }
}
