//! Simulated preference enforcement for automated experimental evaluation.
//!
//! Two one-by-one strategies:
//!   - **Greedy**: always pick the highest-utility violated preference next.
//!   - **AllCombinations**: also one-by-one, but tries every possible ordering
//!     (all permutations) instead of just highest-utility first.
//!
//! Both share the same conflict resolution: drop the lowest-utility preference.
//! Dropped preferences can be re-attempted later (solver state may have changed).

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
    display_resolutions,
};

// =====================================================================
// Strategy enum (CLI flag)
// =====================================================================

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub(crate) enum Strategy {
    /// Always pick the highest-utility violated preference next.
    Greedy,
    /// One-by-one like Greedy, but explores all possible orderings instead of just highest-utility.
    AllCombinations,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strategy::Greedy => write!(f, "greedy"),
            Strategy::AllCombinations => write!(f, "all-combinations"),
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
fn pick_best_mcs(mcses: &[BTreeSet<Tag>], selected_names: &BTreeSet<String>, weights: &BTreeMap<String, i64>) -> Option<usize> {
    mcses.iter().enumerate()
        .filter_map(|(i, mcs)| {
            let dropped = mcs_dropped_prefs(mcs, selected_names);
            if dropped.is_empty() { return None; }
            let min_weight = dropped.iter().map(|n| weights.get(*n).copied().unwrap_or(0)).min().unwrap();
            Some((i, (min_weight, dropped.len(), dropped.iter().min().unwrap().to_string())))
        })
        .min_by_key(|(_, s)| s.clone())
        .map(|(i, _)| i)
}

// =====================================================================
// One-by-one enforcement loop (shared core for both strategies).
//
// Walks through `ordering` and enforces preferences one at a time.
// On conflict, drops the lowest-utility preference. The ordering is
// repeated N times so dropped preferences get re-attempted later.
// =====================================================================

struct PathResult {
    steps: usize,
    outcome: Outcome,
    enforced: Vec<usize>,
}

fn run_one_by_one(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    model: &Model,
    sched: &Sched,
    entries: &[PreferenceEntry],
    weights: &BTreeMap<String, i64>,
    _violated_indices: &[usize],
    plan_cost_obj: Option<IntTerm>,
    optimal_obj_val: IntCst,
    optimal_plan_cost: Option<IntCst>,
    ordering: &[usize],
    path_label: &str,
) -> PathResult {
    let obj = encoding.objectives[0];
    let mut selected_indices: Vec<usize> = Vec::new();
    let mut steps: usize = 0;
    let mut order_pos = 0;

    loop {
        // Exhausted the ordering — done
        if order_pos >= ordering.len() {
            println!("\n[SIM][{}] No more preferences to try.", path_label);
            return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices };
        }

        let next_idx = ordering[order_pos];
        order_pos += 1;

        // Already enforced — skip
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
                    selected_indices.retain(|&i| !prefs_to_drop.contains(entries[i].name.as_str()));
                    if selected_indices.is_empty() {
                        println!("\n[SIM][{}] All selected preferences were dropped.", path_label);
                    }
                }
                None => {
                    break;
                }
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

            // All preferences satisfied — early exit
            if let Some(ref sol) = latest_solution {
                let still_violated = compute_still_violated(entries, &selected_indices, sol);
                if still_violated.is_empty() {
                    println!("\n[SIM][{}] All preferences are now satisfied.", path_label);
                    return PathResult { steps, outcome: Outcome::Accepted, enforced: selected_indices };
                }
            }
        }
    }
}

// =====================================================================
// Greedy ordering — descending utility, repeated N times for re-attempts.
// =====================================================================

fn build_greedy_ordering(violated_indices: &[usize], entries: &[PreferenceEntry], weights: &BTreeMap<String, i64>) -> Vec<usize> {
    let mut sorted: Vec<usize> = violated_indices.to_vec();
    sorted.sort_by(|&a, &b| {
        let wa = weights.get(&entries[a].name).copied().unwrap_or(0);
        let wb = weights.get(&entries[b].name).copied().unwrap_or(0);
        wb.cmp(&wa).then_with(|| entries[a].name.cmp(&entries[b].name))
    });

    let n = sorted.len();
    let mut ordering = Vec::with_capacity(n * n);
    for _ in 0..n {
        ordering.extend_from_slice(&sorted);
    }
    ordering
}

// =====================================================================
// All-combinations — every permutation of violated indices.
// Warning: grows as N!, only practical for small N.
// =====================================================================

fn permutations(items: &[usize]) -> Vec<Vec<usize>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut result = Vec::new();
    for (i, &item) in items.iter().enumerate() {
        let mut rest: Vec<usize> = items.to_vec();
        rest.remove(i);
        for mut perm in permutations(&rest) {
            perm.insert(0, item);
            result.push(perm);
        }
    }
    result
}

/// Repeat a permutation N times for re-attempting dropped preferences.
fn build_combination_ordering_with_retries(perm: &[usize], n_rounds: usize) -> Vec<usize> {
    let mut ordering = Vec::with_capacity(perm.len() * n_rounds);
    for _ in 0..n_rounds {
        ordering.extend_from_slice(perm);
    }
    ordering
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

    match strategy {
        Strategy::Greedy => {
            let ordering = build_greedy_ordering(&violated_indices, &entries, &weights);
            let result = run_one_by_one(
                &mut solver.clone(), encoding, model, sched, &entries, &weights,
                &violated_indices, plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                &ordering, "greedy",
            );
            let _metrics = SimulationMetrics { steps: result.steps, outcome: result.outcome };
            println!("\n[SIM] Greedy path enforced: [{}]",
                result.enforced.iter().map(|&i| entries[i].name.as_str()).collect::<Vec<_>>().join(", "));
        }
        Strategy::AllCombinations => {
            let perms = permutations(&violated_indices);
            let n = violated_indices.len();
            println!("[SIM] Exploring {} paths ({} violated preferences)\n", perms.len(), n);

            let mut best: Option<(usize, usize, Vec<usize>)> = None;
            let mut worst: Option<(usize, usize, Vec<usize>)> = None;

            for (pi, perm) in perms.iter().enumerate() {
                let ordering = build_combination_ordering_with_retries(perm, n);
                let label = format!("path-{}", pi + 1);
                let result = run_one_by_one(
                    &mut solver.clone(), encoding, model, sched, &entries, &weights,
                    &violated_indices, plan_cost_obj, optimal_obj_val, optimal_plan_cost,
                    &ordering, &label,
                );
                let perm_names: Vec<&str> = perm.iter().map(|&i| entries[i].name.as_str()).collect();
                let enforced_names: Vec<&str> = result.enforced.iter().map(|&i| entries[i].name.as_str()).collect();
                println!("\n[SIM] Path {} (order: [{}]): {} steps, outcome: {}, enforced: [{}]",
                    pi + 1,
                    perm_names.join(", "),
                    result.steps,
                    result.outcome,
                    enforced_names.join(", "),
                );

                let is_better = best.as_ref().map_or(true, |(s, _, _)| result.steps < *s);
                if is_better { best = Some((result.steps, pi, result.enforced.clone())); }
                let is_worse = worst.as_ref().map_or(true, |(s, _, _)| result.steps > *s);
                if is_worse { worst = Some((result.steps, pi, result.enforced)); }
            }

            println!("\n===== All-combinations summary =====");
            if let Some((steps, pi, ref enforced)) = best {
                let names: Vec<&str> = enforced.iter().map(|&i| entries[i].name.as_str()).collect();
                println!("  Best:  path {} — {} steps, enforced: [{}]", pi + 1, steps, names.join(", "));
            }
            if let Some((steps, pi, ref enforced)) = worst {
                let names: Vec<&str> = enforced.iter().map(|&i| entries[i].name.as_str()).collect();
                println!("  Worst: path {} — {} steps, enforced: [{}]", pi + 1, steps, names.join(", "));
            }

            if let Some((steps, _, _)) = worst {
                let _metrics = SimulationMetrics { steps, outcome: Outcome::Accepted };
            }
        }
    }
}
