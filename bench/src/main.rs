use anyhow::{Context, Result};
use aries_bench::{
    plot::PlotOptionsBuilder,
    results::{ProblemResults, ResultCollection},
    time_series::TimeSerie,
    *,
};
use clap::{Parser, ValueEnum};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use std::{collections::BTreeMap, f64, fs, ops::AddAssign, rc::Rc, time::Duration};
use std::{fmt::Display, ops::DivAssign};

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
    /// Only consider instances with the given Timeout
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
    #[arg(long, value_enum, default_value = "all")]
    conf: Configuration,
    #[arg(long, default_value = "")]
    out_dir: String,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum Plot {
    Solved,
    Quality,
}
#[derive(Debug, Copy, Clone, ValueEnum)]
enum Configuration {
    All,
    Fjs,
    FjsLag,
    FjsTt,
}
impl Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Configuration::All => "all",
                Configuration::Fjs => "fjs",
                Configuration::FjsLag => "fjslag",
                Configuration::FjsTt => "fjstt",
            }
        )
    }
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

    let (mut filters, mut excludes) = match args.conf {
        Configuration::All => (vec![], vec![]),
        Configuration::Fjs => (vec![], vec!["lag".to_string(), "layout".to_string()]),
        Configuration::FjsLag => (vec!["lag".to_string()], vec![]),
        Configuration::FjsTt => (vec!["layout".to_string()], vec![]),
    };
    filters.extend_from_slice(&args.filter);
    excludes.extend_from_slice(&args.exclude);

    filters.iter().for_each(|f| col.retain(|pb| pb.id().contains(f)));
    excludes.iter().for_each(|f| col.retain(|pb| !pb.id().contains(f)));
    args.timeout
        .iter()
        .for_each(|to| col.retain(|pb| pb.timeout == Duration::from_secs(*to)));
    args.easy.then(|| col = col.clone().easy());
    args.hard.then(|| col = col.clone().hard());

    print_comparison_table(&col, &evaluated, &reference);
    plot(&col, &args.plots, &args.conf.to_string(), &args.out_dir);

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
        println!("Solved: ");
        for (solver, val) in solved {
            println!("  {solver}: {val:.2}");
        }
        // dbg!(solved);
        // let objective = avg(
        //     cur.measures(|_pb, res| res.objective_value.map(|i| i as f64).unwrap_or(f64::NAN)),
        //     |(_pb, solver, count)| (solver, count),
        // );
        // println!("Objective: ");
        // for (solver, val) in objective {
        //     println!("  {solver}: {val:.2}");
        // }
        let ipc = avg(
            cur.measures(|_pb, res| {
                let best = _pb.results.iter().filter_map(|(_s, r)| r.objective_value).min();
                let Some(best) = best else {
                    return 0.0;
                };
                let best = best as f64;

                res.objective_value.map(|i| best / (i as f64)).unwrap_or(0.0)
            }),
            |(_pb, solver, count)| (solver, count),
        );
        println!("IPC: ");
        for (solver, val) in ipc {
            println!("  {solver}: {:.2}", val * 100.0);
        }
        // dbg!(objective);
        // let branches = avg(
        //     cur.measures(|_pb, res| res.metrics.get(&Metric::NumDecisions).copied().unwrap_or(f64::NAN)),
        //     |(_pb, solver, count)| (solver, count),
        // );
        // dbg!(branches);
    }

    Ok(())
}

fn plot(col: &ResultCollection, plots: &[Plot], basename: &str, out_dir: &str) {
    let col = col.clone().with_data_for_all_solvers();
    let plots_options = PlotOptionsBuilder::default()
        .min_x(0.1)
        .log_x(true)
        .x_label("Time (s)".to_string())
        .out_dir(out_dir.to_string());

    for plot in plots {
        match plot {
            Plot::Quality => {
                let opts = plots_options
                    .clone()
                    .title("Quality".to_string())
                    .y_label("IPC Score".to_string())
                    .file(format!("{basename}-quality"))
                    .legenc_loc(plot::LegendLoc::BottomRight)
                    .build()
                    .unwrap();
                let series = col.measures::<TimeSerie>(ipc_hist);
                let results = avg(series, |(_, solver, serie)| (solver, serie));
                plot::plot_cactus(&results, &opts);
            }
            Plot::Solved => {
                let opts = plots_options
                    .clone()
                    .title("Optimality proofs".to_string())
                    .y_label("Solved instances".to_string())
                    .file(format!("{basename}-solved"))
                    .build()
                    .unwrap();
                let series = col.measures::<TimeSerie>(solved_hist);
                let results = sum(series, |(_, solver, serie)| (solver, serie));
                plot::plot_cactus(&results, &opts);
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
    let mut results = col.comparison(main, reference);

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

    results.sort_by_cached_key(|r| r.run.problem.id());

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
