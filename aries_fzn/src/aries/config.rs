use aries::solver::search::activity::ActivityBrancher;
use aries::solver::search::activity::BranchingParams;
use aries::solver::search::combinators::RoundRobin;
use aries::solver::search::SearchControl;
use clap::ValueEnum;

use crate::aries::Solver;

pub type Brancher = Box<dyn SearchControl<usize> + Send>;

/// Aries solving configuration.
#[derive(ValueEnum, Default, Clone, Copy, Debug)]
pub enum Config {
    #[default]
    ActivityMin,
    ActivityMax,
    ActivityMinMax,
}

impl Config {
    /// Return the brancher of the configuration.
    pub fn brancher(&self) -> Brancher {
        match self {
            Config::ActivityMin => {
                let params = BranchingParams {
                    prefer_min_value: true,
                    allowed_conflicts: 100,
                    increase_ratio_for_allowed_conflicts: 1.5,
                };
                let brancher = ActivityBrancher::new_with_params(params);
                Box::new(brancher)
            }
            Config::ActivityMax => {
                let params = BranchingParams {
                    prefer_min_value: false,
                    allowed_conflicts: 100,
                    increase_ratio_for_allowed_conflicts: 1.5,
                };
                let brancher = ActivityBrancher::new_with_params(params);
                Box::new(brancher)
            }
            Config::ActivityMinMax => {
                let min = Self::ActivityMin.brancher();
                let max = Self::ActivityMax.brancher();
                let brancher = RoundRobin::new(100, 1.2, vec![min, max]);
                Box::new(brancher)
            }
        }
    }

    /// Apply the configuration on the given solver.
    pub fn apply(&self, solver: &mut Solver) {
        solver.set_brancher(self.brancher());
    }
}
