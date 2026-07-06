use anyhow::{Context, Result};
use aries_bench::{
    aggregator::{Avg, Sum, avg, sum},
    metric::{Ipc, IpcHist, Solved, SolvedHist},
    plot::PlotOptionsBuilder,
    results::ResultCollection,
    table::*,
    *,
};
use clap::{Parser, ValueEnum};
use std::fmt::Display;
use std::{fs, rc::Rc, time::Duration};

/// Compare set of benchmark results.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing the benchmark results to be used as reference to compute improvements.
    /// The first one is the evaluated solver and the second is used as a reference to compute improvements.
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
    /// Ask to generate the corresponding plot(s)
    #[arg(long = "plot", value_enum)]
    plots: Vec<Plot>,
    /// Ask to generate the corresponding plot(s)
    #[arg(long = "table", value_enum)]
    tables: Vec<Table>,
    /// Only retain problems that were solved by all solvers
    #[arg(long)]
    easy: bool,
    /// Only retain problems that were not solved by at least one solver
    #[arg(long)]
    hard: bool,
    /// Base directory in which to look for planner results.
    #[arg(long = "base-dir", short = 'd')]
    base_directory: Option<String>,
    /// Predefined configuration (filters/excludes). TODO: remove or generalize beyond flexible jobshop
    #[arg(long, value_enum, default_value = "all")]
    conf: Configuration,
    /// Diretory to which plots and tables will be exported
    #[arg(long, default_value = "/tmp")]
    out_dir: String,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum Plot {
    Solved,
    Quality,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum Table {
    Solved,
    Ipc,
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
    results.sort_by_key(|a| a.problem.id());
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

    let reference = solver_path(
        args.solvers
            .get(1)
            .context("At least two solvers should be provided.")?,
    );
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

    for table in &args.tables {
        // common table options
        let opts = TableOptionsBuilder::default()
            .highlight_best(true)
            .out_dir(args.out_dir.clone());

        match *table {
            Table::Solved => {
                print_latex_table(
                    &col,
                    &|_, solver| solver.to_string(),
                    &|pb, _| pb.dirname("data/"),
                    Solved,
                    Sum,
                    |s| format!("{s}"),
                    opts.file(format!("{}-solved.tex", args.conf)).build().unwrap(),
                );
            }
            Table::Ipc => {
                print_latex_table(
                    &col,
                    &|_, solver| solver.to_string(),
                    &|pb, _| pb.dirname("data/"),
                    Ipc,
                    Avg,
                    |s| format!("{:.1}", s * 100.0),
                    opts.file(format!("{}-ipc.tex", args.conf)).build().unwrap(),
                );
            }
        }
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
                let series = col.measures(IpcHist);
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
                let series = col.measures(SolvedHist);
                let results = sum(series, |(_, solver, serie)| (solver, serie));
                plot::plot_cactus(&results, &opts);
            }
        }
    }
}

fn print_comparison_table(col: &ResultCollection, main: &SolverID, reference: &SolverID) {
    use comfy_table::modifiers::UTF8_ROUND_CORNERS;
    use comfy_table::presets::UTF8_FULL;
    use comfy_table::*;
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
            SolveStatus::SolvedUnsat => "Solved(Unsat)",
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

        let conflicts = result.metric(SolverMetric::NumConflicts).map(readable::Int::from);
        let decisions = result.metric(SolverMetric::NumDecisions).map(readable::Int::from);
        let dom_updates = result.metric(SolverMetric::NumDomUpdates).map(readable::Int::from);

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
    println!(
        "Above is a table showing the results of '{main}', using '{reference}' as a reference to compute variations."
    )
}
