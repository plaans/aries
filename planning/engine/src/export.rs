use std::path::Path;

use planx::Res;

use crate::generate::PlanGenerationResult;

pub type SolveStatus = aries_bench_data::SolveStatus;
pub type ReportMetadata = aries_bench_data::Problem;
pub type Report = aries_bench_data::SolveResult;

impl PlanGenerationResult {
    /// Generates a report in the format of `aries-bench`
    pub fn generate_report(&self, metadata: ReportMetadata) -> Report {
        Report {
            problem: metadata,
            status: self.status,
            runtime: self.runtime,
            objective_value: self.objective_value.map(|x| x as i64),
            metrics: Default::default(),
            objective_history: vec![],
        }
        .with_metric(
            aries_bench_data::SolverMetric::NumConflicts,
            self.solver_stats.num_conflicts as f64,
        )
        .with_metric(
            aries_bench_data::SolverMetric::NumDecisions,
            self.solver_stats.num_decisions as f64,
        )
        .with_metric(
            aries_bench_data::SolverMetric::NumDomUpdates,
            self.solver_stats.num_dom_updates as f64,
        )
    }
}

pub(crate) fn export_report_to_dir(report_dir: &Path, report: Report) -> Res<()> {
    report
        .save_to_dir(&report_dir.to_string_lossy())
        .map_err(|e| planx::Message::error(format!("{e}")))
}
