use aries::model::Label;
use aries::solver::search::activity::ActivityBrancher;
use aries::solver::search::activity::BranchingParams;
use aries::solver::search::combinators::RoundRobin;
use aries::solver::search::SearchControl;
use aries::solver::Solver;
use clap::ValueEnum;

/// Aries solving configuration.
#[derive(ValueEnum, Default, Clone, Copy, Debug)]
pub enum Config {
    #[default]
    ActivityMin,
    ActivityMax,
    ActivityMinMax,
}

impl Config {
    /// Return the brancher of the given configuration
    pub fn brancher<Lbl: Label>(&self) -> Box<dyn SearchControl<Lbl> + Send> {
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

    /// Apply the configuration on the given aries solver.
    pub fn apply<Lbl: Label>(&self, solver: &mut Solver<Lbl>) {
        solver.brancher = self.brancher();
    }
}
