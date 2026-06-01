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
fn noop(_: &Solution) {}

/// One preference from the PDDL model, with its solver literals and
/// its satisfaction status in the original optimal plan.
struct PreferenceEntry {
    name: String,
    /// Solver literals that must all hold for this preference to be satisfied.
    lits: Vec<Lit>,
    /// Human-readable display string (e.g. "pref0: at(pkg1, loca)").
    display: String,
    /// Whether this preference is satisfied in the original optimal solution.
    is_satisfied: bool,
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

/// Print a prompt and read one line from stdin.
/// Returns None if stdin is not a terminal (piped input).
fn read_interactive_input(prompt: &str) -> Option<String> {
    if !io::stdin().is_terminal() {
        return None;
    }
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    Some(input)
}

/// Convert a solver `Tag` into a human-readable label for conflict/resolution display.
fn format_tag(tag: &Tag, model: &Model) -> String {
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
fn compute_plan_cost(sched: &Sched, solution: &Solution) -> Option<IntCst> {
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
fn format_cost_delta(new_val: IntCst, base_val: IntCst) -> String {
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
fn print_preference_status(sol: &Solution, entries: &[PreferenceEntry], selected_indices: &[usize]) {
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
fn print_preference_changes(
    sol: &Solution,
    forced_prefs: &[&str],
    satisfied_entries: &[(String, Vec<Lit>, String)],
    unsatisfied: &[(String, Vec<Lit>, String)],
) {
    let mut newly_violated = Vec::new();
    let mut newly_satisfied = Vec::new();
    // Check if any originally-satisfied preference is now violated (side-effect of enforcement)
    for (sat_name, sat_lits, _) in satisfied_entries {
        let now_sat = sat_lits.iter().all(|lit| sol.entails(*lit));
        if !now_sat {
            newly_violated.push(sat_name.as_str());
        }
    }
    // Check if any non-selected violated preference became satisfied as a bonus
    for (unsat_name, unsat_lits, _) in unsatisfied {
        if forced_prefs.contains(&unsat_name.as_str()) {
            continue; // skip the ones we explicitly enforced
        }
        let now_sat = unsat_lits.iter().all(|lit| sol.entails(*lit));
        if now_sat {
            newly_satisfied.push(unsat_name.as_str());
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
    let entries: Vec<PreferenceEntry> = encoding
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
        .collect();

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

    // Print each preference with its status; violated ones are marked with '*'
    for (i, entry) in entries.iter().enumerate() {
        let status = if entry.is_satisfied { "satisfied" } else { "VIOLATED" };
        let marker = if entry.is_satisfied { " " } else { "*" };
        println!("  {}{:>2}. [{}] {}", marker, i + 1, status, entry.display);
    }

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

    // =====================================================================
    // Interaction metrics (for experimental comparison of strategies)
    //
    //   steps   — total user inputs (valid or not). Measures effort.
    //   outcome — how the session ended:
    //     Accepted     — user saw a plan and accepted it (normal exit).
    //     AllSatisfied — the optimized plan satisfies ALL preferences,
    //                    including unselected ones (side-effect of forcing
    //                    a subset that steers the solver to a good region).
    //     GaveUp       — user quit during conflict resolution, before any
    //                    plan was produced.
    //     Cancelled    — stdin closed (Ctrl-D). Default if no other is set.
    //     Exhausted    — user resolved conflicts until no preferences
    //                    remained, then quit instead of re-selecting.
    //
    // Uses Drop to print automatically on any exit path.
    // =====================================================================
    #[derive(Clone, Copy)]
    enum Outcome { Accepted, AllSatisfied, GaveUp, Cancelled, Exhausted }
    impl std::fmt::Display for Outcome {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Outcome::Accepted => write!(f, "accepted"),
                Outcome::AllSatisfied => write!(f, "all-satisfied"),
                Outcome::GaveUp => write!(f, "gave-up"),
                Outcome::Cancelled => write!(f, "cancelled"),
                Outcome::Exhausted => write!(f, "exhausted"),
            }
        }
    }
    struct InteractionMetrics { steps: usize, outcome: Outcome }
    impl Drop for InteractionMetrics {
        fn drop(&mut self) {
            if self.steps > 0 {
                println!("\nInteraction steps: {} (outcome: {})", self.steps, self.outcome);
            }
        }
    }
    // Default outcome is Cancelled — overwritten if the user reaches a
    // meaningful exit point.
    let mut metrics = InteractionMetrics { steps: 0, outcome: Outcome::Cancelled };

    // --- Prompt user to select which violated preferences to enforce ---
    let mut selected_indices: Vec<usize> = loop {
        let input = match read_interactive_input("Which violated preferences do you want to enforce? (e.g. 2,4 or 1-3 or 'all', 'q' to quit): ") {
            Some(input) => input,
            None => return,
        };
        metrics.steps += 1; // [any outcome] (user picks which violated preferences to enforce)

        match parse_selection(&input, entries.len()) {
            Ok(SelectionInput::Cancel) => return,
            Ok(SelectionInput::All) => break violated_indices.clone(),
            Ok(SelectionInput::Indices(idx)) => {
                // Reject selection of already-satisfied preferences
                let non_violated: Vec<usize> = idx.iter().filter(|i| !violated_indices.contains(i)).copied().collect();
                if !non_violated.is_empty() {
                    let names: Vec<String> = non_violated.iter().map(|&i| format!("{} ({})", i + 1, entries[i].name)).collect();
                    println!("  Already satisfied, cannot select: {}. Try again.", names.join(", "));
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

    // Pre-compute tuples for print_preference_changes (which tracks status flips)
    let satisfied_tuples: Vec<(String, Vec<Lit>, String)> = entries
        .iter()
        .filter(|e| e.is_satisfied)
        .map(|e| (e.name.clone(), e.lits.clone(), e.display.clone()))
        .collect();
    let unsatisfied_tuples: Vec<(String, Vec<Lit>, String)> = entries
        .iter()
        .filter(|e| !e.is_satisfied)
        .map(|e| (e.name.clone(), e.lits.clone(), e.display.clone()))
        .collect();

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

        // Feasibility probe: clone solver to avoid side effects, use all enablers
        // INCLUDING cost bound (to test feasibility within the budget).
        let mut probe = solver.clone();
        let all_enabler_lits_probe: Vec<Lit> = probe.enablers().keys().copied().collect();
        let mut assumptions_with_cost = all_enabler_lits_probe;
        assumptions_with_cost.extend(&selected_pref_lits);

        if probe
            .find_optimal_with_assumptions(obj, noop, &assumptions_with_cost)
            .is_some()
        {
            // Selection is feasible within the cost bound — proceed to Phase 1
            break;
        }

        // --- Selection is infeasible within cost bound: explain via MUS/MCS ---
        println!("\nThe selected preferences cannot all be enforced within the cost bound.\n");

        // Set up MUS/MCS analysis: add selected + satisfied preferences as extra assumptions
        // so the MARCO algorithm can identify which subsets conflict.
        let mut explain_solver = solver.clone();
        let mut extra = BTreeMap::new();
        for &idx in &selected_indices {
            for &lit in &entries[idx].lits {
                extra.insert(lit, Tag::EnforcePreference(entries[idx].name.clone()));
            }
        }
        // Also include satisfied preferences — they may appear in conflicts
        // (enforcing a violated pref may conflict with an already-satisfied one)
        for entry in entries.iter().filter(|e| e.is_satisfied) {
            for &lit in &entry.lits {
                extra.insert(lit, Tag::EnforcePreference(entry.name.clone()));
            }
        }

        // Collect MUS (conflicts) and MCS (resolutions) with per-preference limits.
        // Instead of a global cap (e.g. "first 5"), we collect up to PER_PREF_LIMIT
        // MUS and MCS entries for each selected preference, ensuring every preference
        // gets adequate coverage in the explanations.
        const PER_PREF_LIMIT: usize = 2;
        let selected_names_for_collection: BTreeSet<String> = selected_indices
            .iter()
            .map(|&i| entries[i].name.clone())
            .collect();
        let mut mus_count_per_pref: BTreeMap<String, usize> = selected_names_for_collection
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
                    // Count which selected preferences this MUS covers
                    let involved: Vec<&String> = s.iter().filter_map(|t| match t {
                        Tag::EnforcePreference(name) if selected_names_for_collection.contains(name) => Some(name),
                        _ => None,
                    }).collect();
                    // Skip if all involved preferences already have enough MUS entries
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
                        Tag::EnforcePreference(name) if selected_names_for_collection.contains(name) => Some(name),
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
            // Stop once every selected preference has enough MUS and MCS entries
            let all_mus_covered = mus_count_per_pref.values().all(|&c| c >= PER_PREF_LIMIT);
            let all_mcs_covered = mcs_count_per_pref.values().all(|&c| c >= PER_PREF_LIMIT);
            if all_mus_covered && all_mcs_covered {
                break;
            }
        }

        // --- Display conflicts (MUS), split into structural vs budget ---
        // Structural: preferences that are mutually incompatible regardless of cost.
        // Budget: preferences that could coexist but their combined cost exceeds the bound.
        if !muses.is_empty() {
            let has_cost_bound = |mus: &BTreeSet<Tag>| mus.iter().any(|t| matches!(t, Tag::CostBound));
            let structural: Vec<_> = muses.iter().filter(|m| !has_cost_bound(m)).collect();
            let budget: Vec<_> = muses.iter().filter(|m| has_cost_bound(m)).collect();

            let mut conflict_num = 1;

            if !structural.is_empty() {
                println!("  Structural conflicts (incompatible preferences):\n");
                for mus in &structural {
                    let tags: Vec<_> = mus.iter().map(|t| {
                        let label = format_tag(t, model);
                        // Annotate non-selected preferences so the user knows they weren't chosen
                        match t {
                            Tag::EnforcePreference(name) if !selected_names_for_collection.contains(name) => {
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
                    // Filter out CostBound tag — it's implicit in the "exceed cost bound" phrasing
                    let tags: Vec<_> = mus.iter().filter(|t| !matches!(t, Tag::CostBound)).map(|t| {
                        let label = format_tag(t, model);
                        match t {
                            Tag::EnforcePreference(name) if !selected_names_for_collection.contains(name) => {
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

        // --- Display resolutions (MCS) ---
        // Each MCS is a minimal set of assumptions to drop to restore feasibility.
        // We separate tags into "to_drop" (selected preferences) and "side_effects" (other tags).
        let selected_names_set: BTreeSet<&str> = selected_indices
            .iter()
            .map(|&i| entries[i].name.as_str())
            .collect();

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
                        Tag::EnforcePreference(name) if selected_names_set.contains(name.as_str()) => {
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

        // Show currently selected preferences for reference during drop interaction
        println!("  Currently selected preferences:\n");
        for (pos, &idx) in selected_indices.iter().enumerate() {
            println!("    {:>2}. {}", pos + 1, entries[idx].display);
        }
        println!();

        // --- Prompt user to apply a resolution, drop preferences, or relax the cost bound ---
        let has_resolutions = !resolutions.is_empty();
        let prompt = if has_resolutions {
            "Apply a resolution (R1, R2, ...), drop manually (numbers), 'p' to proceed ignoring cost bound, 'q' to quit: "
        } else {
            "Drop preferences (numbers, 'all'), 'p' to proceed ignoring cost bound, 'q' to quit: "
        };

        let user_input = match read_interactive_input(prompt) {
            Some(input) => input,
            None => return,
        };
        metrics.steps += 1; // [GaveUp|Exhausted] (user resolves a conflict: drop, apply resolution, proceed, or quit)
        let trimmed = user_input.trim();

        // Handle "proceed ignoring cost bound" — skip to Phase 2
        if trimmed == "p" || trimmed == "proceed" {
            skip_to_phase2 = true;
            break;
        }

        // Handle resolution selection (e.g. "R1", "R2")
        if has_resolutions && trimmed.to_lowercase().starts_with('r') {
            if let Ok(r_idx) = trimmed[1..].trim().parse::<usize>() {
                if r_idx >= 1 && r_idx <= resolutions.len() {
                    let res = &resolutions[r_idx - 1];
                    // Drop the selected preferences that the MCS says to remove
                    let prefs_to_drop: BTreeSet<&str> = mcses[r_idx - 1]
                        .iter()
                        .filter_map(|tag| match tag {
                            Tag::EnforcePreference(name) if selected_names_set.contains(name.as_str()) => {
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

                    if !res.side_effects.is_empty() {
                        println!("  Note: {} may also be affected", res.side_effects.join(", "));
                    }
                    if selected_indices.is_empty() {
                        println!("\nAll selected preferences were dropped.");
                    }
                    continue;
                }
            }
            println!("  Invalid resolution. Select a single resolution (e.g. R1) or drop preferences by number.");
            continue;
        }

        // Handle manual drop by preference numbers
        match parse_selection(trimmed, selected_indices.len()) {
            // [GaveUp] (user abandoned during conflict resolution without producing a plan)
            Ok(SelectionInput::Cancel) => { metrics.outcome = Outcome::GaveUp; return; }
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
        for (i, entry) in entries.iter().enumerate() {
            let status = if entry.is_satisfied { "satisfied" } else { "VIOLATED" };
            let marker = if entry.is_satisfied { " " } else { "*" };
            println!("  {}{:>2}. [{}] {}", marker, i + 1, status, entry.display);
        }
        println!("\nViolated preferences: {}\n", violated_display.join(", "));

        selected_indices = loop {
            let sel_input = match read_interactive_input(
                "Which violated preferences do you want to enforce? (e.g. 2,4 or 1-3 or 'all', 'q' to quit): ",
            ) {
                Some(input) => input,
                None => return,
            };
            metrics.steps += 1; // [Exhausted→retry] (user re-selects preferences after previous selection was fully dropped)
            match parse_selection(&sel_input, entries.len()) {
                Ok(SelectionInput::Cancel) => return,
                Ok(SelectionInput::All) => break violated_indices.clone(),
                Ok(SelectionInput::Indices(idx)) => {
                    let non_violated: Vec<usize> = idx
                        .iter()
                        .filter(|i| !violated_indices.contains(i))
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
                    break idx;
                }
                Err(e) => {
                    println!("  Error: {}. Try again.", e);
                    continue;
                }
            }
        };
        continue; // back to outer loop with new selection
    }

    // =====================================================================
    // Phase 1: Enforce selected preferences within the original cost bound
    //
    // All enablers (including CostBound) are active, plus the preference
    // literals for the selected set. If the solver finds a solution, it means
    // we can enforce the preferences without exceeding the original budget.
    // Skipped when the user chose 'p' (proceed ignoring cost bound).
    // =====================================================================
    let selected_pref_lits: Vec<Lit> = selected_indices
        .iter()
        .flat_map(|&i| entries[i].lits.iter().copied())
        .collect();
    let selected_names: Vec<&str> = selected_indices
        .iter()
        .map(|&i| entries[i].name.as_str())
        .collect();
    let selected_names_display = selected_names.join(", ");

    let mut latest_solution: Option<Solution> = None;

    if !skip_to_phase2 {
        let all_enabler_lits: Vec<Lit> = solver.enablers().keys().copied().collect();
        let mut assumptions_bounded = all_enabler_lits;
        assumptions_bounded.extend(&selected_pref_lits);

        println!("\n===== Phase 1: Enforce {{ {} }} within cost bound =====\n", selected_names_display);

        let bounded_sol = solver.find_optimal_with_assumptions(obj, noop, &assumptions_bounded);

        if let Some(ref sol) = bounded_sol {
            print_preference_status(sol, &entries, &selected_indices);
            let new_obj = sol.eval(obj).unwrap();
            let new_cost = compute_plan_cost(sched, sol);
            println!("    Objective: {}", format_cost_delta(new_obj, optimal_obj_val));
            if let (Some(fc), Some(oc)) = (new_cost, optimal_plan_cost) {
                println!("    Plan cost: {}", format_cost_delta(fc, oc));
            }
            print_preference_changes(sol, &selected_names, &satisfied_tuples, &unsatisfied_tuples);
            println!("    Resulting plan:\n{}", encoding.plan(sol));
        }

        latest_solution = bounded_sol;
    }

    // =====================================================================
    // Phase 2: Relax cost bound and minimize plan cost
    //
    // Entered when Phase 1 failed or when the user chose 'p' (proceed
    // ignoring cost bound) during conflict resolution.
    // =====================================================================
    if latest_solution.is_none() {
        println!("  Infeasible within cost bound.");
        if let Some(cost_obj) = plan_cost_obj {
            let mut assumptions_unbounded: Vec<Lit> = solver
                .enablers()
                .iter()
                .filter(|(_, tag)| !matches!(tag, Tag::CostBound))
                .map(|(lit, _)| *lit)
                .collect();
            assumptions_unbounded.extend(&selected_pref_lits);

            println!("\n===== Phase 2: Relaxing cost bound (minimizing plan cost) =====\n");
            if let Some(sol) = solver.find_optimal_with_assumptions(cost_obj, noop, &assumptions_unbounded) {
                print_preference_status(&sol, &entries, &selected_indices);
                let new_cost = compute_plan_cost(sched, &sol);
                if let (Some(fc), Some(oc)) = (new_cost, optimal_plan_cost) {
                    println!("    Plan cost: {}", format_cost_delta(fc, oc));
                }
                let new_obj = sol.eval(obj).unwrap();
                println!("    Objective: {}", format_cost_delta(new_obj, optimal_obj_val));
                print_preference_changes(&sol, &selected_names, &satisfied_tuples, &unsatisfied_tuples);
                println!("    Resulting plan:\n{}", encoding.plan(&sol));
                latest_solution = Some(sol);
            } else {
                println!("    Structurally infeasible: cannot be satisfied even without a cost bound.");
            }
        }
    }

    // =====================================================================
    // Continuation: add more preferences, start fresh, or accept and quit
    //
    // After Phase 1/2, we check which preferences are still violated in
    // the produced plan. If there are none, the user has achieved full
    // satisfaction and we exit. Otherwise, we offer three options:
    //   (a) Add more — cumulative: keep the currently enforced set and
    //       pick additional violated preferences to enforce on top.
    //   (n) New selection — forget all previous enforcements and start
    //       from scratch with the original violated preferences list.
    //   (q) Quit — accept the current plan as-is.
    //
    // After (a) or (n), selected_indices is updated and the outer loop
    // repeats: conflict detection → Phase 1/2 → continuation.
    // =====================================================================

    // Find preferences that are violated in the latest plan AND are not
    // already in the enforced set. These are candidates for "add more".
    let still_violated: Vec<usize> = if let Some(ref sol) = latest_solution {
        entries
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                !selected_indices.contains(i)
                    && !e.lits.iter().all(|lit| sol.entails(*lit))
            })
            .map(|(i, _)| i)
            .collect()
    } else {
        // Both phases failed — no solution to evaluate, nothing to add to
        Vec::new()
    };

    // "Add more" is only possible if we have a plan and there are still
    // violated preferences the user hasn't selected yet.
    let can_add_more = latest_solution.is_some() && !still_violated.is_empty();

    // If every preference is satisfied, there's nothing left to explore
    if latest_solution.is_some() && still_violated.is_empty() {
        println!("\nAll preferences are now satisfied.");
        // [AllSatisfied] (plan satisfies all preferences, including unselected ones — side-effect of optimization)
        metrics.outcome = Outcome::AllSatisfied;
        break;
    }

    // Show which preferences remain violated so the user can decide
    if can_add_more {
        println!("\nStill violated:");
        for &idx in &still_violated {
            println!("  {:>2}. {}", idx + 1, entries[idx].display);
        }
    }

    // Prompt the user for their next action.
    // The loop keeps asking until a valid option is given.
    // `break 'a'` / `break 'n'` return a char literal from the loop.
    let action: char = loop {
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
            "a" | "add" if can_add_more => break 'a',
            "n" | "new" => break 'n',
            _ => {
                if can_add_more {
                    println!("  Enter 'a', 'n', or 'q'.");
                } else {
                    println!("  Enter 'n' or 'q'.");
                }
            }
        }
    };

    if action == 'a' {
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
        let new_picks: Vec<usize> = loop {
            let sel_input = match read_interactive_input(
                "Which preferences to add? ('q' to accept current plan): ",
            ) {
                Some(input) => input,
                None => return,
            };
            metrics.steps += 1; // [Accepted|AllSatisfied] (user picks additional preferences to enforce on top of current ones)
            match parse_selection(&sel_input, entries.len()) {
                Ok(SelectionInput::Cancel) => return,
                Ok(SelectionInput::All) => break still_violated.clone(),
                Ok(SelectionInput::Indices(idx)) => {
                    // Reject indices that aren't in still_violated
                    let invalid: Vec<usize> = idx
                        .iter()
                        .filter(|i| !still_violated.contains(i))
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
        for (i, entry) in entries.iter().enumerate() {
            let status = if entry.is_satisfied { "satisfied" } else { "VIOLATED" };
            let marker = if entry.is_satisfied { " " } else { "*" };
            println!("  {}{:>2}. [{}] {}", marker, i + 1, status, entry.display);
        }
        println!("\nViolated preferences: {}\n", violated_display.join(", "));

        // Same selection loop as the initial one at the start of the function
        selected_indices = loop {
            let sel_input = match read_interactive_input(
                "Which violated preferences do you want to enforce? (e.g. 2,4 or 1-3 or 'all', 'q' to quit): ",
            ) {
                Some(input) => input,
                None => return,
            };
            metrics.steps += 1; // [Accepted|AllSatisfied] (user starts fresh and picks preferences from scratch)
            match parse_selection(&sel_input, entries.len()) {
                Ok(SelectionInput::Cancel) => return,
                Ok(SelectionInput::All) => break violated_indices.clone(),
                Ok(SelectionInput::Indices(idx)) => {
                    let non_violated: Vec<usize> = idx
                        .iter()
                        .filter(|i| !violated_indices.contains(i))
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
                    break idx;
                }
                Err(e) => {
                    println!("  Error: {}. Try again.", e);
                    continue;
                }
            }
        };
    }
    // After (a) or (n), selected_indices has been updated.
    // The outer loop continues: conflict detection → Phase 1/2 → back here.

    } // end outer interaction loop
}
