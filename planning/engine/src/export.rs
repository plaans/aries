use std::path::Path;

use aries_solver::prelude::*;

use planx::Res;

use crate::generate::PlanGenerationResult;

pub type SolveStatus = aries_bench_data::SolveStatus;
pub type ReportMetadata = aries_bench_data::Problem;
pub type Report = aries_bench_data::SolveResult;

fn default_report_metadata() -> ReportMetadata {
    ReportMetadata {
        name: "".to_string(),
        timeout: std::time::Duration::ZERO,
        flags: Default::default(),
    }
}

pub fn make_default_report_from_plangen_result<Lbl>(
    plangen_result: &PlanGenerationResult,
    solver: &Solver<Lbl>,
) -> Report {
    Report {
        problem: default_report_metadata(),
        status: plangen_result.status,
        runtime: plangen_result.runtime,
        objective_value: plangen_result.objective_value.map(|x| x as i64),
        metrics: Default::default(),
        objective_history: vec![],
    }
    .with_metric(
        aries_bench_data::SolverMetric::NumConflicts,
        solver.stats.num_conflicts as f64,
    )
    .with_metric(
        aries_bench_data::SolverMetric::NumDecisions,
        solver.stats.num_decisions as f64,
    )
    .with_metric(
        aries_bench_data::SolverMetric::NumDomUpdates,
        solver.stats.num_dom_updates as f64,
    )
}

pub(crate) fn export_report_to_dir(report_dir: &Path, report: Report) -> Res<()> {
    report
        .save_to_dir(&report_dir.to_string_lossy())
        .map_err(|e| planx::Message::error(format!("{e}")))
}
