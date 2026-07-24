use std::path::Path;

use aries_bench_data::{SolveStatus, SolverMetric};

use aries_solver::{core::state::Evaluable, prelude::*};

use planx::Res;

pub(crate) fn export_report_to_file<Lbl: aries_solver::model::Label>(
    report_file: &Path,
    solution: Option<Solution>,
    objective: LinTerm,
    runtime: std::time::Duration,
    solver: &Solver<Lbl>,
) -> Res<()> {
    let status = SolveStatus::Solved(solution.is_some());

    let result = aries_bench_data::SolveResult {
        problem: aries_bench_data::Problem {
            name: report_file
                .file_stem()
                .ok_or(planx::Message::error("invalid export file"))?
                .to_string_lossy()
                .to_string(),
            timeout: std::time::Duration::MAX,
            flags: Default::default(),
        },
        status,
        runtime,
        objective_value: solution
            .as_ref()
            .and_then(|sol| objective.evaluate(sol).map(|x| x.into())),
        metrics: Default::default(),
        objective_history: vec![],
    }
    .with_metric(SolverMetric::NumConflicts, solver.stats.num_conflicts as f64)
    .with_metric(SolverMetric::NumDecisions, solver.stats.num_decisions as f64)
    .with_metric(SolverMetric::NumDomUpdates, solver.stats.num_dom_updates as f64);

    result
        .save_to_file(&report_file.to_string_lossy())
        .map_err(|e| planx::Message::error(format!("{e}")))?;

    Ok(())
}
