use aries_planning::chronicles::VarLabel;

pub mod encode;
pub mod encoding;
pub mod fmt;
pub mod forward_search;
pub mod solver;

pub type Model = aries::model::Model<VarLabel>;
pub type Solver = aries::solver::solver::Solver<VarLabel>;
pub type ParSolver = aries::solver::parallel_solver::ParSolver<VarLabel>;
