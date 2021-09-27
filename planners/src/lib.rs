pub mod encode;
pub mod encoding;
pub mod fmt;
pub mod forward_search;

/// Label of a variable
pub type Var = String;

pub type Model = aries_model::Model<Var>;
pub type Solver = aries_solver::solver::Solver<Var>;
pub type ParSolver = aries_solver::parallel_solver::ParSolver<Var>;
