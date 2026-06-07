use aries_planning::chronicles::VarLabel;

pub mod encode;
pub mod encoding;
pub mod fmt;
pub mod search;
pub mod solver;

pub type Model = aries_solver::model::Model<VarLabel>;
pub type Solver = aries_solver::solver::Solver<VarLabel>;
pub type ParSolver = aries_solver::solver::parallel::ParSolver<VarLabel>;
