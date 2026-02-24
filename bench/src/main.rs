use anyhow::{Context, Result};
use aries_bench::{
    results::{ProblemResults, ResultCollection},
    time_series::TimeSerie,
    *,
};
use clap::{Parser, ValueEnum};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use std::ops::DivAssign;
use std::{collections::BTreeMap, f64, fs, ops::AddAssign, rc::Rc, time::Duration};

/// Compare set of benchmark results.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing the benchmark results to be used as reference to compute improvements.
    solvers: Vec<String>,
    /// Only consider instances whose ID contains the given string
    #[arg(short, long)]
    filter: Vec<String>,
    /// Only consider instances whose ID does not contain the given string
    #[arg(short, long)]
    exclude: Vec<String>,
    /// Only consider instances with the givne Timeout
    #[arg(short, long)]
    timeout: Option<u64>,
    #[arg(short, long = "plot", value_enum)]
    plots: Vec<Plot>,
    /// Only retain problems that were solved by all solvers
    #[arg(long)]
    easy: bool,
    /// Only retain problems that were not solved by at least one solver
    #[arg(long)]
    hard: bool,
    /// Base directory in which to look for planner results.
    #[arg(long = "base-dir", short = 'd')]
    base_directory: Option<String>,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum Plot {
    Solved,
    Quality,
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
    let args = Args::parse();

    let solver_path = |solver_path: &str| {
        if let Some(base) = args.base_directory.as_ref() {
            format!("{base}{solver_path}")
        } else {
            solver_path.to_string()
        }
    };

    let reference = solver_path(&args.solvers[1]);
    let evaluated = solver_path(&args.solvers[0]);

    let mut col = ResultCollection::default();
    for solver in &args.solvers {
        let solver = solver_path(solver);
        let results = results_from_dir(&solver)?;
        col.add_solver(solver, results);
    }

    args.filter.iter().for_each(|f| col.retain(|pb| pb.id().contains(f)));
    args.exclude.iter().for_each(|f| col.retain(|pb| !pb.id().contains(f)));
    args.timeout
        .iter()
        .for_each(|to| col.retain(|pb| pb.timeout == Duration::from_secs(*to)));
    args.easy.then(|| col = col.clone().easy());
    args.hard.then(|| col = col.clone().hard());

    print_comparison_table(&col, &evaluated, &reference);
    plot(&col, &args.plots);

    let col = col.with_data_for_all_solvers();
    let filters = [Some("lag"), Some("lay"), None];

    let mut base = col.clone();
    for f in filters {
        let cur = if let Some(filter) = f {
            let mut cur = base.clone();
            cur.retain(|pb| pb.id().contains(filter));
            base.retain(|pb| !pb.id().contains(filter));
            cur
        } else {
            base.clone()
        };

        println!("\n == Filter: {f:?} == \n");
        let solved = sum(
            cur.measures(|_pb, res| if res.status == SolveStatus::Solved { 1 } else { 0 }),
            |(_pb, solver, count)| (solver, count),
        );
        dbg!(solved);
        let objective = avg(
            cur.measures(|_pb, res| res.objective_value.map(|i| i as f64).unwrap_or(f64::NAN)),
            |(_pb, solver, count)| (solver, count),
        );
        dbg!(objective);
        // let branches = avg(
        //     cur.measures(|_pb, res| res.metrics.get(&Metric::NumDecisions).copied().unwrap_or(f64::NAN)),
        //     |(_pb, solver, count)| (solver, count),
        // );
        // dbg!(branches);
    }

    Ok(())
}

fn plot(col: &ResultCollection, plots: &[Plot]) {
    let col = col.clone().with_data_for_all_solvers();

    for plot in plots {
        match plot {
            Plot::Quality => {
                let series = col.measures::<TimeSerie>(ipc_hist);
                let results = avg(series, |(_, solver, serie)| (solver, serie));
                plot::plot_cactus(&results);
            }
            Plot::Solved => {
                let series = col.measures::<TimeSerie>(solved_hist);
                let results = sum(series, |(_, solver, serie)| (solver, serie));
                plot::plot_cactus(&results);
            }
        }
    }
}

fn ipc_hist(runs: &ProblemResults, run: &SolveResult) -> TimeSerie {
    let Some(best) = runs.results.values().filter_map(|r| r.objective_value).min() else {
        return TimeSerie::constant(0.0, Duration::ZERO, runs.problem.timeout);
    };
    run.ipc_history(best)
}
fn solved_hist(_runs: &ProblemResults, run: &SolveResult) -> TimeSerie {
    run.solved_hist()
}
fn sum<Measure, Key, Value>(
    measures: impl Iterator<Item = Measure>,
    kv: impl Fn(Measure) -> (Key, Value),
) -> BTreeMap<Key, Value>
where
    Key: Ord,
    Value: AddAssign<Value>,
{
    let mut results = BTreeMap::default();
    for measure in measures {
        let (k, v) = kv(measure);
        if let Some(prev) = results.get_mut(&k) {
            *prev += v;
        } else {
            results.insert(k, v);
        }
    }
    results
}
fn avg<Measure, Key, Value>(
    measures: impl Iterator<Item = Measure>,
    kv: impl Fn(Measure) -> (Key, Value),
) -> BTreeMap<Key, Value>
where
    Key: Ord + Clone,
    Value: AddAssign<Value> + DivAssign<f64>,
{
    let mut counts: BTreeMap<_, i32> = BTreeMap::new();
    let mut results = BTreeMap::new();
    for measure in measures {
        let (k, v) = kv(measure);
        *counts.entry(k.clone()).or_default() += 1;
        if let Some(prev) = results.get_mut(&k) {
            *prev += v;
        } else {
            results.insert(k, v);
        }
    }
    for (k, v) in results.iter_mut() {
        *v /= counts[k] as f64;
    }
    results
}

fn print_comparison_table(col: &ResultCollection, main: &SolverID, reference: &SolverID) {
    let results = col.comparison(main, reference);

    // Create and configure comfy table
    let mut table = Table::new();
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
        ]);

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
            .map(|r| format!("{:.2}", r));

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
}
