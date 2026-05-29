use std::{cmp::Ordering, time::Duration};

use aries_bench_data::{SolveResult, SolveStatus};

use crate::{results::ProblemResults, time_series::TimeSerie};

pub trait Metric: Copy + Clone + 'static {
    type T;
    fn short_name(&self) -> String;

    fn compute(&self, result: &SolveResult, all_results_for_pb: &ProblemResults) -> Self::T;

    fn compare(&self, _m1: Self::T, _m2: Self::T) -> Ordering {
        Ordering::Equal
    }
}

#[derive(Copy, Clone)]
pub struct Solved;

/// Number of solved problems
impl Metric for Solved {
    type T = u32;

    fn short_name(&self) -> String {
        "solved".to_string()
    }

    fn compute(&self, result: &SolveResult, _all_results_for_pb: &ProblemResults) -> Self::T {
        if result.status == SolveStatus::Solved { 1 } else { 0 }
    }

    fn compare(&self, m1: Self::T, m2: Self::T) -> Ordering {
        // direct comparison but larger is better
        m1.cmp(&m2)
    }
}

/// Number of solved problems over time.
#[derive(Copy, Clone)]
pub struct SolvedHist;

impl Metric for SolvedHist {
    type T = TimeSerie;

    fn short_name(&self) -> String {
        "solved-hist".to_string()
    }

    fn compute(&self, result: &SolveResult, _all_results_for_pb: &ProblemResults) -> Self::T {
        let mut hist = vec![(Duration::ZERO, 0.0)];
        if result.status == SolveStatus::Solved {
            hist.push((result.runtime, 1.0));
        }
        TimeSerie::from_constant_per_part(hist, result.problem.timeout)
    }
}

/// IPC score over time.
///
/// The IPC score is defined as the `C*/C` where `C*` is the best (i.e. smallest) cost among all runs
/// for this problem while `C` is the cost of the result.
/// When no result is available, the cost is interpreted as infinity and the score is 0.
#[derive(Copy, Clone)]
pub struct Ipc;

impl Metric for Ipc {
    type T = f64;

    fn short_name(&self) -> String {
        "ipc".to_string()
    }

    fn compute(&self, result: &SolveResult, all_results_for_pb: &ProblemResults) -> Self::T {
        let best = all_results_for_pb
            .results
            .values()
            .filter_map(|r| r.objective_value)
            .min();
        let Some(best) = best else {
            return 0.0;
        };
        let best = best as f64;

        result.objective_value.map(|i| best / (i as f64)).unwrap_or(0.0)
    }

    fn compare(&self, m1: Self::T, m2: Self::T) -> Ordering {
        if let Some(ord) = m1.partial_cmp(&m2) {
            return ord;
        }
        // we couldn't get a valid comparison, meaning that the one of the elements is NaN
        if m1.is_nan() && m2.is_nan() {
            Ordering::Equal
        } else if m2.is_nan() {
            Ordering::Less // m1 is better
        } else if m1.is_nan() {
            Ordering::Greater // m2 is better
        } else {
            unreachable!("Unhandled case of float comparison...")
        }
    }
}

/// IPC score over time.
///
/// The IPC score is defined as the `C*/C` where `C*` is the best (i.e. smallest) cost among all runs
/// for this problem while `C` is the cost of the result.
/// When no result is available, the cost is interpreted as infinity and the score is 0.
#[derive(Copy, Clone)]
pub struct IpcHist;

impl Metric for IpcHist {
    type T = TimeSerie;

    fn short_name(&self) -> String {
        "ipc-hist".to_string()
    }

    fn compute(&self, result: &SolveResult, all_results_for_pb: &ProblemResults) -> Self::T {
        let Some(best) = all_results_for_pb
            .results
            .values()
            .filter_map(|r| r.objective_value)
            .min()
        else {
            // no problem with a solution, return a cosntant zero score
            return TimeSerie::constant(0.0, Duration::ZERO, all_results_for_pb.problem.timeout);
        };
        let best = best as f64;
        // from 0 until the first solution, the IPC score is zero
        let mut hist = vec![(Duration::ZERO, 0.0)];
        for measure in &result.objective_history {
            let x = best / measure.objective as f64;
            assert!(x.is_finite(), "{best} / {}", measure.objective);
            hist.push((measure.timestamp, best / measure.objective as f64));
        }
        if let Some(final_obj) = result.objective_value {
            hist.push((result.runtime, best / final_obj as f64));
        }
        TimeSerie::from_constant_per_part(hist, result.problem.timeout)
    }
}
