use anyhow::{Context, Result};
use aries_bench::{comp::RunWithRef, *};
use clap::Parser;
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
    let reference_results = results_from_dir(&args.reference)?; // TODO

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

    // Print header
    println!(
        "{:<60} {:<10} {:>15} {:>15} {:>15} {:>15} {:>15}",
        "Problem", "Status", "Objective", "Runtime", "Conflicts", "Decisions", "DomUpdates"
    );
    println!("{}", "-".repeat(140));

    // Print each result with metrics
    for result in results {
        let problem_name = result.run.problem.id();
        let status = match result.run.status {
            SolveStatus::Solved => "Solved",
            SolveStatus::Timeout => "Timeout",
        };
        let objective = result.objective();
        let runtime = result
            .measure(|r| Some(r.runtime.as_secs_f64()))
            .map(readable::Float::new_2_point);

        let conflicts = result.metric(Metric::NumConflicts).to_string();
        let decisions = result.metric(Metric::NumDecisions).to_string();
        let dom_updates = result.metric(Metric::NumDomUpdates).to_string();

        println!(
            "{:<60} {:<10} {:>15} {:>15} {:>15} {:>15} {:>15}",
            problem_name, status, objective, runtime, conflicts, decisions, dom_updates
        );
    }

    Ok(())
}
