use anyhow::{Context, Result};
use aries_bench::{comp::RunWithRef, *};
use clap::Parser;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use std::{fs, rc::Rc};

/// Benchmark results analyzer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(help = "Path to directory with benchmark results")]
    reference: String,
    /// Directory containing benchmark result JSON files
    #[arg(help = "Path to directory with benchmark results")]
    directory: String,
}

fn results_from_dir(directory: &str) -> Result<Vec<Rc<SolveResult>>> {
    // Read all JSON files from the directory
    let entries = fs::read_dir(directory).context("Failed to read directory")?;

    // Collect all results for sorting
    let mut results = Vec::new();

    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        // Only process JSON files
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        // Try to load and parse the result
        match SolveResult::load_from_file(path.to_str().unwrap()) {
            Ok(result) => results.push(Rc::new(result)),
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            }
        }
    }

    // Sort results by problem ID
    results.sort_by(|a, b| a.problem.id().cmp(&b.problem.id()));
    Ok(results)
}

fn main() -> anyhow::Result<()> {
    // Parse command line arguments using clap
    let args = Args::parse();
    let directory = args.directory;

    let results = results_from_dir(&directory)?;
    let reference_results = results_from_dir(&args.reference)?;

    let results: Vec<RunWithRef> = results
        .into_iter()
        .map(|run| {
            let reference = reference_results
                .iter()
                .find(|ref_run| run.problem.id() == ref_run.problem.id())
                .map(Rc::clone);
            RunWithRef { run, reference }
        })
        .collect();

    // Create and configure comfy table
    let mut table = Table::new();

    // Set table style with borders and alignment
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Problem").set_alignment(comfy_table::CellAlignment::Left),
            Cell::new("Status").set_alignment(comfy_table::CellAlignment::Left),
            Cell::new("Objective").set_alignment(comfy_table::CellAlignment::Right),
            Cell::new("Runtime").set_alignment(comfy_table::CellAlignment::Right),
            Cell::new("Conflicts").set_alignment(comfy_table::CellAlignment::Right),
            Cell::new("Decisions").set_alignment(comfy_table::CellAlignment::Right),
            Cell::new("DomUpdates").set_alignment(comfy_table::CellAlignment::Right),
        ])
        .set_content_arrangement(ContentArrangement::Dynamic);

    // Add each result as a row
    for result in results {
        let problem_name = result.run.problem.id();
        let status = match result.run.status {
            SolveStatus::Solved => "Solved",
            SolveStatus::Timeout => "Timeout",
        };
        let status_color = match result.reference.as_ref().map(|r| r.status) {
            Some(s) if s > result.run.status => Color::Green,
            Some(s) if s < result.run.status => Color::Red,
            Some(_) => Color::Grey,
            None => Color::White,
        };
        let status = Cell::new(status).fg(status_color);
        let objective = result.objective();
        let runtime = result
            .measure(|r| Some(r.runtime.as_secs_f64()))
            .map(readable::Float::new_2_point);

        let conflicts = result.metric(Metric::NumConflicts).map(readable::Int::from);
        let decisions = result.metric(Metric::NumDecisions).map(readable::Int::from);
        let dom_updates = result.metric(Metric::NumDomUpdates).map(readable::Int::from);

        table.add_row(vec![
            Cell::new(problem_name).set_alignment(comfy_table::CellAlignment::Left),
            status,
            objective.cell(),
            runtime.cell(),
            conflicts.cell(),
            decisions.cell(),
            dom_updates.cell(),
        ]);
    }

    // Print the table
    println!("{}", table);

    Ok(())
}
