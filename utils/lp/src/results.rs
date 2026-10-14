//! Exports various types capturing the status of an LP or a solver.

use crate::{Solution, Variable};

/// Results of a successful run of an [`Lp::solve`](crate::Lp::solve) call (solving the LP to an optimal).
#[derive(Debug)]
pub enum SolveStatus<'solver> {
    /// Solver terminated at an optimal solution.
    ///
    /// Provides an interface for extracting the optimal solution.
    Optimal(Solution<'solver>),
    /// Solver terminated by proving that the problem has no feasible solution.
    Infeasible(InfeasibilityReason),
    /// Solver terminated by proving that the problem is unbounded (admits feasible solution with arbitrary high objective).
    Unbounded,
}

impl<'a> SolveStatus<'a> {
    /// Returns the solution if the LP has a finite optimal solution (not infeasible nor unbounded).
    pub fn solution(self) -> Option<Solution<'a>> {
        match self {
            SolveStatus::Optimal(solution) => Some(solution),
            SolveStatus::Infeasible(_) | SolveStatus::Unbounded => None,
        }
    }

    /// Returns true iff the LP is infeasible
    pub fn infeasible(&self) -> bool {
        match self {
            SolveStatus::Infeasible(_) => true,
            SolveStatus::Optimal(_) | SolveStatus::Unbounded => false,
        }
    }

    /// Returns true if the LP is unbounded
    pub fn unbounded(&self) -> bool {
        match self {
            SolveStatus::Unbounded => true,
            SolveStatus::Optimal(_) | SolveStatus::Infeasible(_) => false,
        }
    }

    /// Returns true, iff the LP has an finite optimal solution and that the one found is accepted by the given predicate.
    pub fn is_optimal_and(&'a self, pred: impl FnOnce(&'a Solution<'a>) -> bool) -> bool {
        match self {
            SolveStatus::Optimal(s) => pred(s),
            _ => false,
        }
    }
}

/// Result for a successful check of satisfiability.
#[derive(Debug)]
pub enum FeasibilityStatus {
    /// The LP is feasible, i.e., admits at least one solution (ignoring the objective).
    Feasible,
    /// The LP admits no solution (ignoring the objective).
    Infeasible(InfeasibilityReason),
}

impl FeasibilityStatus {
    /// Returns `true` iff the LP is feasible.
    pub fn feasible(&self) -> bool {
        match self {
            FeasibilityStatus::Feasible => true,
            FeasibilityStatus::Infeasible(_) => false,
        }
    }

    /// Returns `true` iff the LP has no feasible solution.
    pub fn infeasible(&self) -> bool {
        !self.feasible()
    }
}

/// A Farkas certificate that can be used as a proof of unsatisfiability.
pub type InfeasibilityCertificate = Vec<f64>;

/// Explain where the infeasibility comes from.
#[derive(Debug, Clone)]
pub enum InfeasibilityReason {
    /// Obvious infeasibility, that can be typically detected at model construction
    IncompatibleBounds(Variable),
    /// Infeasibility was proven by the solver (typically after a number of simplex pivots).
    /// This variant contains a Farkas certificate that can be used to certify the infeasibility.
    Certified(InfeasibilityCertificate),
}

/// Denotes the reason why a feasibility check could not be concluded.
#[derive(Debug, Clone)]
pub enum SolverFailure {
    /// Instability detected in floating point operations, preventing the solver to reach a conclusion.
    Unstable,
}

/// Error denoting the presence of numerical instability.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NumericalErrors;
