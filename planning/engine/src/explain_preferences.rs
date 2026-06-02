//! Interactive preference enforcement for plan optimization.
//!
//! Given an optimal plan that violates some PDDL preferences, this module lets
//! the user interactively select which violated preferences to enforce, detects
//! conflicts among the selection (via MUS/MCS analysis), guides the user through
//! conflict resolution, and finally produces an optimized plan that satisfies the
//! chosen preferences.
//!
//! The process has two phases:
//!   Phase 1 — enforce the selected preferences while respecting the original cost bound.
//!   Phase 2 — if Phase 1 is infeasible, relax the cost bound and minimize plan cost instead.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, IsTerminal, Write as _},
};

use aries::{
    prelude::*,
    solver::musmcs::MusMcs,
};
use aries_plan_engine::encode::{encoding::Encoding, tags::Tag};

use planx::{Model, SimpleGoal};
use timelines::{EffectOp, IntTerm, Sched, explain::ExplainableSolver};

/// No-op callback for `find_optimal_with_assumptions` (we only need the solution).
pub(crate) fn noop(_: &Solution) {}

/// One preference from the PDDL model, with its solver literals and
/// its satisfaction status in the original optimal plan.
pub(crate) struct PreferenceEntry {
    pub(crate) name: String,
    /// Solver literals that must all hold for this preference to be satisfied.
    pub(crate) lits: Vec<Lit>,
    /// Human-readable display string (e.g. "pref0: at(pkg1, loca)").
    pub(crate) display: String,
    /// Whether this preference is satisfied in the original optimal solution.
    pub(crate) is_satisfied: bool,
}

/// Parsed result of user input when selecting preferences or dropping them.
enum SelectionInput {
    /// User selected specific indices (0-based).
    Indices(Vec<usize>),
    /// User typed "all".
    All,
    /// User wants to quit / cancel (empty input, "q", "quit", "none").
    Cancel,
}

// =====================================================================
// Interaction metrics (for experimental comparison of strategies)
//
//   steps   — total user inputs (valid or not). Measures effort.
//   outcome — how the session ended:
//     Accepted     — session ended with a plan (normal exit, or all
//                    preferences satisfied automatically).
//     Cancelled    — user quit or stdin closed before producing a plan.
//     Exhausted    — user resolved conflicts until no preferences
//                    remained, then quit instead of re-selecting.
//
// Uses Drop to print automatically on any exit path.
// =====================================================================

#[derive(Clone, Copy)]
pub(crate) enum Outcome { Accepted, Cancelled, Exhausted }

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Accepted => write!(f, "accepted"),
            Outcome::Cancelled => write!(f, "cancelled"),
            Outcome::Exhausted => write!(f, "exhausted"),
        }
    }
}

struct InteractionMetrics { steps: usize, outcome: Outcome }

/// Post-plan user action: add more preferences or start a new selection.
enum Action { Add, New }

impl Drop for InteractionMetrics {
    fn drop(&mut self) {
        if self.steps > 0 {
            println!("\nInteraction steps: {} (outcome: {})", self.steps, self.outcome);
        }
    }
}

/// Parse a user input string into a selection of indices.
/// Accepts comma/space-separated numbers, ranges (e.g. "1-3"), "all", or quit keywords.
fn parse_selection(input: &str, max_index: usize) -> Result<SelectionInput, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "none" || trimmed == "q" || trimmed == "quit" {
        return Ok(SelectionInput::Cancel);
    }
    if trimmed == "all" {
        return Ok(SelectionInput::All);
    }

    let mut indices = BTreeSet::new();
    for token in trimmed.split(|c: char| c == ',' || c.is_whitespace()) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((start_s, end_s)) = token.split_once('-') {
            // Range: e.g. "2-5"
            let start: usize = start_s
                .trim()
                .parse()
                .map_err(|_| format!("Invalid number: '{}'", start_s.trim()))?;
            let end: usize = end_s
                .trim()
                .parse()
                .map_err(|_| format!("Invalid number: '{}'", end_s.trim()))?;
            if start == 0 || end == 0 {
                return Err("Indices start at 1".to_string());
            }
            if start > end {
                return Err(format!("Invalid range: {}-{}", start, end));
            }
            if end > max_index {
                return Err(format!("Index {} out of range (max: {})", end, max_index));
            }
            for i in start..=end {
                indices.insert(i - 1);
            }
        } else {
            // Single number
            let idx: usize = token
                .parse()
                .map_err(|_| format!("Invalid number: '{}'", token))?;
            if idx == 0 || idx > max_index {
                return Err(format!("Index {} out of range (1-{})", idx, max_index));
            }
            indices.insert(idx - 1);
        }
    }

    if indices.is_empty() {
        Ok(SelectionInput::Cancel)
    } else {
        Ok(SelectionInput::Indices(indices.into_iter().collect()))
    }
}

/// Print the numbered preference list with satisfaction status.
pub(crate) fn print_preference_list(entries: &[PreferenceEntry]) {
    for (i, entry) in entries.iter().enumerate() {
        let status = if entry.is_satisfied { "satisfied" } else { "VIOLATED" };
        let marker = if entry.is_satisfied { " " } else { "*" };
        println!("  {}{:>2}. [{}] {}", marker, i + 1, status, entry.display);
    }
}

/// Prompt the user to select violated preferences to enforce.
/// Returns None if the user cancels (quit / Ctrl-D), Some(indices) otherwise.
fn prompt_violated_selection(
    entries: &[PreferenceEntry],
    violated_indices: &[usize],
    prompt: &str,
    steps: &mut usize,
) -> Option<Vec<usize>> {
    let violated_set: BTreeSet<usize> = violated_indices.iter().copied().collect();
    loop {
        let input = read_interactive_input(prompt)?;
        *steps += 1;
        match parse_selection(&input, entries.len()) {
            Ok(SelectionInput::Cancel) => return None,
            Ok(SelectionInput::All) => return Some(violated_indices.to_vec()),
            Ok(SelectionInput::Indices(idx)) => {
                let non_violated: Vec<usize> = idx
                    .iter()
                    .filter(|i| !violated_set.contains(i))
                    .copied()
                    .collect();
                if !non_violated.is_empty() {
                    let names: Vec<String> = non_violated
                        .iter()
                        .map(|&i| format!("{} ({})", i + 1, entries[i].name))
                        .collect();
                    println!(
                        "  Already satisfied, cannot select: {}. Try again.",
                        names.join(", ")
                    );
                    continue;
                }
                return Some(idx);
            }
            Err(e) => {
                println!("  Error: {}. Try again.", e);
            }
        }
    }
}

/// Print a prompt and read one line from stdin.
/// Returns None if stdin is not a terminal, on EOF (Ctrl-D), or on I/O error.
fn read_interactive_input(prompt: &str) -> Option<String> {
    if !io::stdin().is_terminal() {
        return None;
    }
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    let bytes_read = io::stdin().read_line(&mut input).ok()?;
    if bytes_read == 0 {
        println!();
        return None;
    }
    Some(input)
}

/// Convert a solver `Tag` into a human-readable label for conflict/resolution display.
pub(crate) fn format_tag(tag: &Tag, model: &Model) -> String {
    match tag {
        Tag::EnforceGoal(goal_idx) => {
            let goal = &model.goals[*goal_idx];
            format!("goal: {}", &model.env / goal)
        }
        Tag::Support { .. } => "support".to_string(),
        Tag::CostBound => "cost bound".to_string(),
        Tag::EnforcePreference(name) => name.clone(),
    }
}

/// Compute the total plan cost by summing all present `total-cost` step effects.
pub(crate) fn compute_plan_cost(sched: &Sched, solution: &Solution) -> Option<IntCst> {
    let mut total: IntCst = 0;
    let mut found = false;
    for eff in sched.effects.iter() {
        if eff.state_var.fluent == "total-cost" {
            if let EffectOp::Step(step_val) = eff.operation {
                if solution.entails(eff.prez) {
                    total += solution.eval(step_val)?;
                    found = true;
                }
            }
        }
    }
    found.then_some(total)
}

/// Format a value with its delta from a baseline (e.g. "8 (base: 7, +1 extra)").
pub(crate) fn format_cost_delta(new_val: IntCst, base_val: IntCst) -> String {
    let delta = new_val - base_val;
    if delta > 0 {
        format!("{} (base: {}, +{} extra)", new_val, base_val, delta)
    } else if delta < 0 {
        format!("{} (base: {}, improved by {})", new_val, base_val, -delta)
    } else {
        format!("{} (base: {}, no change)", new_val, base_val)
    }
}

/// Print the status of every preference in a resulting plan solution.
///
/// Each preference gets one of these labels based on two axes:
///   was it selected for enforcement? × is it satisfied in the new solution?
///
///   | Selected? | Was satisfied? | Now satisfied? | Label     |
///   |-----------|----------------|----------------|-----------|
///   | yes       | —              | yes            | ENFORCED  | successfully enforced
///   | yes       | —              | no             | FAILED    | selected but could not be enforced
///   | no        | yes            | yes            | satisfied | was and remains satisfied
///   | no        | no             | yes            | gained    | bonus: not requested but now satisfied
///   | no        | yes            | no             | RELAXED   | side effect: had to sacrifice this one
///   | no        | no             | no             | violated  | was and remains violated (not selected)
pub(crate) fn print_preference_status(sol: &Solution, entries: &[PreferenceEntry], selected_indices: &[usize]) {
    let selected_set: BTreeSet<usize> = selected_indices.iter().copied().collect();
    println!("    Preference status in resulting plan:\n");
    for (i, entry) in entries.iter().enumerate() {
        let now_sat = entry.lits.iter().all(|lit| sol.entails(*lit));
        let was_sat = entry.is_satisfied;
        let label = if selected_set.contains(&i) {
            if now_sat { "ENFORCED " } else { "FAILED   " }
        } else if now_sat {
            if was_sat { "satisfied" } else { "gained   " }
        } else if was_sat {
            "RELAXED  "
        } else {
            "violated "
        };
        println!("      [{}] {}", label, entry.display);
    }
    println!();
}

/// Print a summary of which preferences changed status compared to the original plan.
/// Only shows preferences whose satisfaction flipped (newly relaxed or newly gained).
pub(crate) fn print_preference_changes(
    sol: &Solution,
    forced_prefs: &[&str],
    entries: &[PreferenceEntry],
) {
    let mut newly_violated = Vec::new();
    let mut newly_satisfied = Vec::new();
    for entry in entries {
        let now_sat = entry.lits.iter().all(|lit| sol.entails(*lit));
        if entry.is_satisfied && !now_sat {
            // Originally satisfied, now violated (side-effect of enforcement)
            newly_violated.push(entry.name.as_str());
        } else if !entry.is_satisfied && now_sat && !forced_prefs.contains(&entry.name.as_str()) {
            // Originally violated, now satisfied, and not explicitly enforced (bonus)
            newly_satisfied.push(entry.name.as_str());
        }
    }
    if !newly_violated.is_empty() || !newly_satisfied.is_empty() {
        println!("    Decision:");
        if !newly_violated.is_empty() {
            println!("      Relaxed: {}", newly_violated.join(", "));
        }
        if !newly_satisfied.is_empty() {
            println!("      Also gained: {}", newly_satisfied.join(", "));
        }
    }
}

// =====================================================================
// Shared pipeline helpers (used by both interactive and simulated modes)
// =====================================================================

/// Build preference entries from the encoding and model, tagged with satisfaction status.
pub(crate) fn build_preference_entries(
    encoding: &Encoding,
    model: &Model,
    optimal_solution: &Solution,
) -> Vec<PreferenceEntry> {
    encoding
        .preferences
        .iter()
        .map(|(name, lits)| {
            let is_satisfied = lits.iter().all(|lit| optimal_solution.entails(*lit));
            let pref = model.preferences.iter().find(|p| p.name.to_string() == *name);
            let display = pref
                .map(|p| {
                    let expr_str = match &p.goal.goal_expression {
                        SimpleGoal::HoldsDuring(_, expr_id)
                        | SimpleGoal::SometimeDuring(_, expr_id)
                        | SimpleGoal::AtMostOnceDuring(_, expr_id) => {
                            format!("{}", &model.env / *expr_id)
                        }
                        _ => format!("{}", &model.env / &p.goal),
                    };
                    format!("{}: {}", p.name, expr_str)
                })
                .unwrap_or_else(|| name.clone());
            PreferenceEntry {
                name: name.clone(),
                lits: lits.clone(),
                display,
                is_satisfied,
            }
        })
        .collect()
}

/// Test whether the selected preferences are jointly feasible within the cost bound.
pub(crate) fn probe_feasibility(
    solver: &ExplainableSolver<Tag>,
    obj: LinTerm,
    selected_pref_lits: &[Lit],
) -> bool {
    let mut probe = solver.clone();
    let all_enabler_lits: Vec<Lit> = probe.enablers().keys().copied().collect();
    let mut assumptions = all_enabler_lits;
    assumptions.extend(selected_pref_lits);
    probe.find_optimal_with_assumptions(obj, noop, &assumptions).is_some()
}

/// Collected MUS (conflicts) and MCS (resolutions) from the MARCO algorithm.
pub(crate) struct MusMcsResult {
    pub muses: Vec<BTreeSet<Tag>>,
    pub mcses: Vec<BTreeSet<Tag>>,
}

/// Run MUS/MCS analysis with per-preference coverage limits.
pub(crate) fn collect_mus_mcs(
    solver: &ExplainableSolver<Tag>,
    entries: &[PreferenceEntry],
    selected_indices: &[usize],
    selected_names: &BTreeSet<String>,
) -> MusMcsResult {
    let mut explain_solver = solver.clone();
    let mut extra = BTreeMap::new();
    for &idx in selected_indices {
        for &lit in &entries[idx].lits {
            extra.insert(lit, Tag::EnforcePreference(entries[idx].name.clone()));
        }
    }
    for entry in entries.iter().filter(|e| e.is_satisfied) {
        for &lit in &entry.lits {
            extra.insert(lit, Tag::EnforcePreference(entry.name.clone()));
        }
    }

    const PER_PREF_LIMIT: usize = 2;
    let mut mus_count_per_pref: BTreeMap<String, usize> = selected_names
        .iter()
        .map(|n| (n.clone(), 0))
        .collect();
    let mut mcs_count_per_pref: BTreeMap<String, usize> = mus_count_per_pref.clone();
    let mut muses: Vec<BTreeSet<Tag>> = Vec::new();
    let mut mcses: Vec<BTreeSet<Tag>> = Vec::new();
    for result in explain_solver.explain_unsat_with_filter(
        |t| matches!(t, Tag::EnforceGoal(_) | Tag::CostBound),
        &extra,
    ) {
        match &result {
            MusMcs::Mus(s) => {
                let involved: Vec<&String> = s.iter().filter_map(|t| match t {
                    Tag::EnforcePreference(name) if selected_names.contains(name.as_str()) => Some(name),
                    _ => None,
                }).collect();
                let dominated = involved.iter().all(|n| mus_count_per_pref[n.as_str()] >= PER_PREF_LIMIT);
                if !dominated {
                    for n in &involved {
                        *mus_count_per_pref.get_mut(n.as_str()).unwrap() += 1;
                    }
                    muses.push(s.clone());
                }
            }
            MusMcs::Mcs(s) => {
                let involved: Vec<&String> = s.iter().filter_map(|t| match t {
                    Tag::EnforcePreference(name) if selected_names.contains(name.as_str()) => Some(name),
                    _ => None,
                }).collect();
                let dominated = involved.iter().all(|n| mcs_count_per_pref[n.as_str()] >= PER_PREF_LIMIT);
                if !dominated {
                    for n in &involved {
                        *mcs_count_per_pref.get_mut(n.as_str()).unwrap() += 1;
                    }
                    mcses.push(s.clone());
                }
            }
        }
        let all_mus_covered = mus_count_per_pref.values().all(|&c| c >= PER_PREF_LIMIT);
        let all_mcs_covered = mcs_count_per_pref.values().all(|&c| c >= PER_PREF_LIMIT);
        if all_mus_covered && all_mcs_covered {
            break;
        }
    }
    MusMcsResult { muses, mcses }
}

/// Display MCS resolutions: each MCS is a minimal set of assumptions to drop
/// to restore feasibility. Tags are split into "to_drop" (selected preferences)
/// and "side_effects" (goals, cost bound, satisfied preferences).
pub(crate) fn display_resolutions(
    mcses: &[BTreeSet<Tag>],
    selected_names: &BTreeSet<String>,
    model: &Model,
) -> usize {
    struct Resolution {
        to_drop: Vec<String>,
        side_effects: Vec<String>,
    }
    let resolutions: Vec<Resolution> = mcses
        .iter()
        .map(|mcs| {
            let mut to_drop = Vec::new();
            let mut side_effects = Vec::new();
            for tag in mcs.iter() {
                let label = format_tag(tag, model);
                match tag {
                    Tag::EnforcePreference(name) if selected_names.contains(name.as_str()) => {
                        to_drop.push(label);
                    }
                    _ => {
                        side_effects.push(label);
                    }
                }
            }
            Resolution { to_drop, side_effects }
        })
        .filter(|r| !r.to_drop.is_empty() || !r.side_effects.is_empty())
        .collect();

    if !resolutions.is_empty() {
        println!("  Proposed resolutions:\n");
        for (i, res) in resolutions.iter().enumerate() {
            let mut parts = Vec::new();
            if !res.to_drop.is_empty() {
                parts.push(format!("drop {}", res.to_drop.join(", ")));
            }
            if !res.side_effects.is_empty() {
                parts.push(format!("relax {}", res.side_effects.join(", ")));
            }
            println!("    R{}: {}", i + 1, parts.join("; "));
        }
        println!();
    }
    resolutions.len()
}

/// Display MUS conflicts, split into structural (incompatible) vs budget (exceed cost bound).
pub(crate) fn display_conflicts(
    muses: &[BTreeSet<Tag>],
    selected_names: &BTreeSet<String>,
    model: &Model,
) {
    if muses.is_empty() {
        return;
    }
    let has_cost_bound = |mus: &BTreeSet<Tag>| mus.iter().any(|t| matches!(t, Tag::CostBound));
    let structural: Vec<_> = muses.iter().filter(|m| !has_cost_bound(m)).collect();
    let budget: Vec<_> = muses.iter().filter(|m| has_cost_bound(m)).collect();
    let mut conflict_num = 1;
    if !structural.is_empty() {
        println!("  Structural conflicts (incompatible preferences):\n");
        for mus in &structural {
            let tags: Vec<_> = mus.iter().map(|t| {
                let label = format_tag(t, model);
                match t {
                    Tag::EnforcePreference(name) if !selected_names.contains(name.as_str()) => {
                        format!("{} (satisfied)", label)
                    }
                    _ => label,
                }
            }).collect();
            println!("    C{}: {}", conflict_num, tags.join(" + "));
            conflict_num += 1;
        }
        println!();
    }
    if !budget.is_empty() {
        println!("  Budget conflicts (exceed cost bound):\n");
        for mus in &budget {
            let tags: Vec<_> = mus.iter().filter(|t| !matches!(t, Tag::CostBound)).map(|t| {
                let label = format_tag(t, model);
                match t {
                    Tag::EnforcePreference(name) if !selected_names.contains(name.as_str()) => {
                        format!("{} (satisfied)", label)
                    }
                    _ => label,
                }
            }).collect();
            let verb = if tags.len() > 1 { "exceed" } else { "exceeds" };
            println!("    C{}: {} {} cost bound", conflict_num, tags.join(" + "), verb);
            conflict_num += 1;
        }
        println!();
    }
}

/// Run Phase 1: enforce selected preferences within the original cost bound.
/// Returns the solution if feasible.
pub(crate) fn run_phase1(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    sched: &Sched,
    entries: &[PreferenceEntry],
    selected_indices: &[usize],
    selected_pref_lits: &[Lit],
    obj: LinTerm,
    optimal_obj_val: IntCst,
    optimal_plan_cost: Option<IntCst>,
) -> Option<Solution> {
    let selected_names_list: Vec<&str> = selected_indices
        .iter()
        .map(|&i| entries[i].name.as_str())
        .collect();
    let selected_names_display = selected_names_list.join(", ");

    let all_enabler_lits: Vec<Lit> = solver.enablers().keys().copied().collect();
    let mut assumptions_bounded = all_enabler_lits;
    assumptions_bounded.extend(selected_pref_lits);

    println!("\n===== Phase 1: Enforce {{ {} }} within cost bound =====\n", selected_names_display);

    let bounded_sol = solver.find_optimal_with_assumptions(obj, noop, &assumptions_bounded);

    if let Some(ref sol) = bounded_sol {
        print_preference_status(sol, entries, selected_indices);
        let new_obj = sol.eval(obj).unwrap();
        let new_cost = compute_plan_cost(sched, sol);
        println!("    Objective: {}", format_cost_delta(new_obj, optimal_obj_val));
        if let (Some(fc), Some(oc)) = (new_cost, optimal_plan_cost) {
            println!("    Plan cost: {}", format_cost_delta(fc, oc));
        }
        print_preference_changes(sol, &selected_names_list, entries);
        println!("    Resulting plan:\n{}", encoding.plan(sol));
    }

    bounded_sol
}

/// Run Phase 2: relax cost bound and minimize plan cost.
/// Returns the solution if feasible.
pub(crate) fn run_phase2(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    sched: &Sched,
    entries: &[PreferenceEntry],
    selected_indices: &[usize],
    selected_pref_lits: &[Lit],
    obj: LinTerm,
    optimal_obj_val: IntCst,
    optimal_plan_cost: Option<IntCst>,
    plan_cost_obj: IntTerm,
) -> Option<Solution> {
    let selected_names_list: Vec<&str> = selected_indices
        .iter()
        .map(|&i| entries[i].name.as_str())
        .collect();

    println!("  Infeasible within cost bound.");

    let mut assumptions_unbounded: Vec<Lit> = solver
        .enablers()
        .iter()
        .filter(|(_, tag)| !matches!(tag, Tag::CostBound))
        .map(|(lit, _)| *lit)
        .collect();
    assumptions_unbounded.extend(selected_pref_lits);

    println!("\n===== Phase 2: Relaxing cost bound (minimizing plan cost) =====\n");
    if let Some(sol) = solver.find_optimal_with_assumptions(plan_cost_obj, noop, &assumptions_unbounded) {
        print_preference_status(&sol, entries, selected_indices);
        let new_cost = compute_plan_cost(sched, &sol);
        if let (Some(fc), Some(oc)) = (new_cost, optimal_plan_cost) {
            println!("    Plan cost: {}", format_cost_delta(fc, oc));
        }
        let new_obj = sol.eval(obj).unwrap();
        println!("    Objective: {}", format_cost_delta(new_obj, optimal_obj_val));
        print_preference_changes(&sol, &selected_names_list, entries);
        println!("    Resulting plan:\n{}", encoding.plan(&sol));
        Some(sol)
    } else {
        println!("    Structurally infeasible: cannot be satisfied even without a cost bound.");
        None
    }
}

/// Compute which preferences are still violated in the latest solution
/// and are not in the currently selected set.
pub(crate) fn compute_still_violated(
    entries: &[PreferenceEntry],
    selected_indices: &[usize],
    solution: &Solution,
) -> Vec<usize> {
    let selected_set: BTreeSet<usize> = selected_indices.iter().copied().collect();
    entries
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            !selected_set.contains(i)
                && !e.lits.iter().all(|lit| solution.entails(*lit))
        })
        .map(|(i, _)| i)
        .collect()
}

/// Main entry point: interactive preference enforcement loop.
///
/// Flow overview:
///   1. Classify all preferences as satisfied/violated in the optimal plan.
///   2. Display them as a numbered list and prompt the user to select violated ones to enforce.
///   3. Check joint feasibility of the selection (ignoring cost bound).
///      - If infeasible: run MUS/MCS analysis, show conflicts & resolutions, let user drop
///        preferences, and loop back to the feasibility check.
///      - If feasible: proceed to Phase 1.
///   4. Phase 1: try to enforce within the original cost bound.
///   5. Phase 2 (fallback): if Phase 1 fails, relax the cost bound and minimize plan cost.
pub(crate) fn interactive_preference_enforcement(
    solver: &mut ExplainableSolver<Tag>,
    encoding: &Encoding,
    model: &Model,
    sched: &Sched,
    optimal_solution: &Solution,
    phase_assumptions: &[Lit],
    plan_cost_obj: Option<IntTerm>,
) {
    if !io::stdin().is_terminal() {
        println!("Interactive mode requires a terminal (stdin must not be a pipe).");
        return;
    }

    if encoding.preferences.is_empty() {
        return;
    }

    // --- Baseline values from the optimal solution ---
    let obj = encoding.objectives[0];
    let optimal_obj_val = optimal_solution.eval(obj).unwrap();
    let optimal_plan_cost = compute_plan_cost(sched, optimal_solution);

    // --- Build preference entries with satisfaction status ---
    let entries = build_preference_entries(encoding, model, optimal_solution);

    // --- Display initial preference overview ---
    println!("\n===== Preference satisfaction (interactive mode) =====\n");
    println!("Optimal objective value: {}", optimal_obj_val);
    if let Some(cost) = optimal_plan_cost {
        println!("Optimal plan cost: {}", cost);
    }
    println!();

    let violated_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.is_satisfied)
        .map(|(i, _)| i)
        .collect();

    print_preference_list(&entries);

    if violated_indices.is_empty() {
        println!("\nAll preferences are satisfied.");
        return;
    }

    let violated_display: Vec<String> = violated_indices
        .iter()
        .map(|&i| format!("{}", i + 1))
        .collect();
    println!(
        "\nViolated preferences: {}\n",
        violated_display.join(", ")
    );

    // Default outcome is Cancelled — overwritten if the user reaches a meaningful exit point.
    let mut metrics = InteractionMetrics { steps: 0, outcome: Outcome::Cancelled };

    // --- Prompt user to select which violated preferences to enforce ---
    // [any outcome] (user picks which violated preferences to enforce)
    let mut selected_indices = match prompt_violated_selection(
        &entries,
        &violated_indices,
        "Which violated preferences do you want to enforce? (e.g. 2,4 or 1-3 or 'all', 'q' to quit): ",
        &mut metrics.steps,
    ) {
        Some(idx) => idx,
        None => return,
    };

    // Lock in the phase assumptions (goals, supports) permanently so they can't be relaxed
    solver.enforce_permanent(phase_assumptions);

    // =====================================================================
    // Outer interaction loop: select → resolve conflicts → enforce → repeat
    //
    // After producing a plan, the user can:
    //   (a) add more preferences to enforce (cumulative — keeps the previously
    //       enforced ones and extends the set with new picks),
    //   (n) start a new selection from scratch (forgets all previous enforcements),
    //   (q) accept the current plan and quit.
    //
    // Each iteration runs the full pipeline: conflict detection → Phase 1/2 → results.
    // The solver state is NOT permanently modified by preference enforcement (only
    // phase_assumptions are permanent); preferences are passed as ephemeral assumptions,
    // so each iteration naturally starts clean.
    // =====================================================================
    loop {

    // =====================================================================
    // Conflict detection and resolution loop
    //
    // We check if the selected set of preferences is jointly feasible
    // (ignoring the cost bound — structural feasibility only).
    // If not, we use MUS/MCS analysis to explain WHY and offer resolutions,
    // then let the user drop conflicting preferences and try again.
    // =====================================================================
    let mut skip_to_phase2 = false;
    loop {
        if selected_indices.is_empty() {
            break;
        }

        let selected_pref_lits: Vec<Lit> = selected_indices
            .iter()
            .flat_map(|&i| entries[i].lits.iter().copied())
            .collect();
        let selected_names: BTreeSet<String> = selected_indices
            .iter()
            .map(|&i| entries[i].name.clone())
            .collect();

        if probe_feasibility(solver, obj, &selected_pref_lits) {
            break;
        }

        // --- Selection is infeasible within cost bound: explain via MUS/MCS ---
        println!("\nThe selected preferences cannot all be enforced within the cost bound.\n");

        let mus_mcs = collect_mus_mcs(solver, &entries, &selected_indices, &selected_names);
        let muses = mus_mcs.muses;
        let mcses = mus_mcs.mcses;

        display_conflicts(&muses, &selected_names, model);

        let num_resolutions = display_resolutions(&mcses, &selected_names, model);

        // Show currently selected preferences for reference during drop interaction
        println!("  Currently selected preferences:\n");
        for (pos, &idx) in selected_indices.iter().enumerate() {
            println!("    {:>2}. {}", pos + 1, entries[idx].display);
        }
        println!();

        // --- Prompt user to apply a resolution, drop preferences, or relax the cost bound ---
        let has_resolutions = num_resolutions > 0;
        let prompt = if has_resolutions {
            "Apply a resolution (R1, R2, ...), drop manually (numbers), 'p' to proceed ignoring cost bound, 'q' to quit: "
        } else {
            "Drop preferences (numbers, 'all'), 'p' to proceed ignoring cost bound, 'q' to quit: "
        };

        let user_input = match read_interactive_input(prompt) {
            Some(input) => input,
            None => return,
        };
        metrics.steps += 1; // [Cancelled|Exhausted] (user resolves a conflict: drop, apply resolution, proceed, or quit)
        let trimmed = user_input.trim();

        // Handle "proceed ignoring cost bound" — skip to Phase 2
        if trimmed == "p" || trimmed == "proceed" {
            skip_to_phase2 = true;
            break;
        }

        // Handle resolution selection (e.g. "R1", "R2")
        if has_resolutions && trimmed.to_lowercase().starts_with('r') {
            if let Some(after_r) = trimmed.get(1..) {
                if let Ok(r_idx) = after_r.trim().parse::<usize>() {
                    if r_idx >= 1 && r_idx <= num_resolutions {
                        // Drop the selected preferences that the MCS says to remove
                        let prefs_to_drop: BTreeSet<&str> = mcses[r_idx - 1]
                            .iter()
                            .filter_map(|tag| match tag {
                                Tag::EnforcePreference(name) if selected_names.contains(name.as_str()) => {
                                    Some(name.as_str())
                                }
                                _ => None,
                            })
                            .collect();

                        selected_indices.retain(|&i| {
                            let keep = !prefs_to_drop.contains(entries[i].name.as_str());
                            if !keep {
                                println!("  Dropped: {}", entries[i].display);
                            }
                            keep
                        });

                        let side_effects: Vec<String> = mcses[r_idx - 1].iter()
                            .filter(|tag| !matches!(tag, Tag::EnforcePreference(name) if selected_names.contains(name.as_str())))
                            .map(|tag| format_tag(tag, model))
                            .collect();
                        if !side_effects.is_empty() {
                            println!("  Note: {} may also be affected", side_effects.join(", "));
                        }
                        if selected_indices.is_empty() {
                            println!("\nAll selected preferences were dropped.");
                        }
                        continue;
                    }
                }
            }
            println!("  Invalid resolution. Select a single resolution (e.g. R1) or drop preferences by number.");
            continue;
        }

        // Handle manual drop by preference numbers
        match parse_selection(trimmed, selected_indices.len()) {
            // [Cancelled] (user abandoned during conflict resolution without producing a plan)
            Ok(SelectionInput::Cancel) => { metrics.outcome = Outcome::Cancelled; return; }
            Ok(SelectionInput::All) => {
                selected_indices.clear();
                println!("All preferences dropped.");
            }
            Ok(SelectionInput::Indices(to_drop)) => {
                let mut to_drop_sorted = to_drop;
                to_drop_sorted.sort_unstable();
                // Remove in reverse order to preserve indices
                for &drop_pos in to_drop_sorted.iter().rev() {
                    let removed = &entries[selected_indices[drop_pos]];
                    println!("  Dropped: {}", removed.display);
                    selected_indices.remove(drop_pos);
                }
                if selected_indices.is_empty() {
                    println!("\nAll selected preferences were dropped.");
                }
            }
            Err(e) => {
                println!("  Error: {}. Try again.", e);
                continue;
            }
        }
    }

    // If all preferences were dropped during conflict resolution,
    // offer a new selection instead of terminating.
    if selected_indices.is_empty() {
        loop {
            let input = match read_interactive_input(
                "\nWhat next? (n = new selection, q = quit): ",
            ) {
                Some(input) => input,
                None => return,
            };
            metrics.steps += 1; // [Exhausted] (user decides whether to re-select or quit after dropping all preferences)
            match input.trim() {
                // [Exhausted] (all preferences dropped and user quits)
                "q" | "quit" => { metrics.outcome = Outcome::Exhausted; return; }
                "n" | "new" => break,
                _ => println!("  Enter 'n' or 'q'."),
            }
        }

        println!();
        print_preference_list(&entries);
        println!("\nViolated preferences: {}\n", violated_display.join(", "));

        // [Exhausted→retry] (user re-selects preferences after previous selection was fully dropped)
        selected_indices = match prompt_violated_selection(
            &entries,
            &violated_indices,
            "Which violated preferences do you want to enforce? (e.g. 2,4 or 1-3 or 'all', 'q' to quit): ",
            &mut metrics.steps,
        ) {
            Some(idx) => idx,
            None => return,
        };
        continue; // back to outer loop with new selection
    }

    let selected_pref_lits: Vec<Lit> = selected_indices
        .iter()
        .flat_map(|&i| entries[i].lits.iter().copied())
        .collect();

    let mut latest_solution: Option<Solution> = None;

    if !skip_to_phase2 {
        latest_solution = run_phase1(
            solver, encoding, sched, &entries, &selected_indices,
            &selected_pref_lits, obj, optimal_obj_val, optimal_plan_cost,
        );
    }

    if latest_solution.is_none() {
        if let Some(cost_obj) = plan_cost_obj {
            latest_solution = run_phase2(
                solver, encoding, sched, &entries, &selected_indices,
                &selected_pref_lits, obj, optimal_obj_val, optimal_plan_cost, cost_obj,
            );
        }
    }

    let still_violated: Vec<usize> = if let Some(ref sol) = latest_solution {
        compute_still_violated(&entries, &selected_indices, sol)
    } else {
        Vec::new()
    };

    // "Add more" is only possible if we have a plan and there are still
    // violated preferences the user hasn't selected yet.
    let can_add_more = latest_solution.is_some() && !still_violated.is_empty();

    // If every preference is satisfied, there's nothing left to explore
    if latest_solution.is_some() && still_violated.is_empty() {
        println!("\nAll preferences are now satisfied.");
        metrics.outcome = Outcome::Accepted;
        break;
    }

    // Show which preferences remain violated so the user can decide
    if can_add_more {
        println!("\nStill violated:");
        for &idx in &still_violated {
            println!("  {:>2}. {}", idx + 1, entries[idx].display);
        }
    }

    let action = loop {
        let prompt = if can_add_more {
            "\nWhat next? (a = add more, n = new selection, q = accept and quit): "
        } else {
            "\nWhat next? (n = new selection, q = quit): "
        };
        let input = match read_interactive_input(prompt) {
            Some(input) => input,
            None => return,
        };
        metrics.steps += 1; // [Accepted] (user decides next action after seeing the plan: add more, new selection, or accept)
        match input.trim() {
            // [Accepted] (user saw a plan and accepted it)
            "q" | "quit" => { metrics.outcome = Outcome::Accepted; return; }
            "a" | "add" if can_add_more => break Action::Add,
            "n" | "new" => break Action::New,
            _ => {
                if can_add_more {
                    println!("  Enter 'a', 'n', or 'q'.");
                } else {
                    println!("  Enter 'n' or 'q'.");
                }
            }
        }
    };

    if matches!(action, Action::Add) {
        // --- Add more (cumulative) ---
        // The user keeps the already-enforced preferences and picks
        // additional ones from those still violated in the latest plan.
        // After selection, the new picks are appended to selected_indices
        // and the outer loop re-runs conflict detection + Phase 1/2
        // with the combined set.
        let enforced_display = selected_indices
            .iter()
            .map(|&i| entries[i].name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "\nSelect additional preferences to enforce (currently enforcing: {}):\n",
            enforced_display
        );
        for &idx in &still_violated {
            println!("  *{:>2}. [violated] {}", idx + 1, entries[idx].display);
        }
        println!();

        // Selection loop: same logic as the initial selection but only
        // allows picking from still_violated (not already-enforced or satisfied)
        let still_violated_set: BTreeSet<usize> = still_violated.iter().copied().collect();
        let new_picks: Vec<usize> = loop {
            let sel_input = match read_interactive_input(
                "Which preferences to add? ('q' to accept current plan): ",
            ) {
                Some(input) => input,
                None => return,
            };
            metrics.steps += 1; // [Accepted] (user picks additional preferences to enforce on top of current ones)
            match parse_selection(&sel_input, entries.len()) {
                Ok(SelectionInput::Cancel) => return,
                Ok(SelectionInput::All) => break still_violated.clone(),
                Ok(SelectionInput::Indices(idx)) => {
                    // Reject indices that aren't in still_violated
                    let invalid: Vec<usize> = idx
                        .iter()
                        .filter(|i| !still_violated_set.contains(i))
                        .copied()
                        .collect();
                    if !invalid.is_empty() {
                        let names: Vec<String> = invalid
                            .iter()
                            .map(|&i| format!("{} ({})", i + 1, entries[i].name))
                            .collect();
                        println!("  Not available: {}. Try again.", names.join(", "));
                        continue;
                    }
                    break idx;
                }
                Err(e) => {
                    println!("  Error: {}. Try again.", e);
                    continue;
                }
            }
        };
        // Extend (not replace) — the previously enforced preferences stay
        selected_indices.extend(new_picks);
    } else {
        // --- New selection (from scratch) ---
        // Forget all previous enforcements and show the original preference
        // overview. The user selects from the originally-violated preferences
        // as if starting the interaction for the first time.
        // Since preferences are enforced via ephemeral assumptions (not
        // permanent solver constraints), the solver state is already clean.
        selected_indices.clear();
        println!();
        print_preference_list(&entries);
        println!("\nViolated preferences: {}\n", violated_display.join(", "));

        // [Accepted] (user starts fresh and picks preferences from scratch)
        selected_indices = match prompt_violated_selection(
            &entries,
            &violated_indices,
            "Which violated preferences do you want to enforce? (e.g. 2,4 or 1-3 or 'all', 'q' to quit): ",
            &mut metrics.steps,
        ) {
            Some(idx) => idx,
            None => return,
        };
    }
    // After (a) or (n), selected_indices has been updated.
    // The outer loop continues: conflict detection → Phase 1/2 → back here.

    } // end outer interaction loop
}
