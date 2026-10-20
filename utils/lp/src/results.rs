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

/// A certificate produced by the solver: the multipliers of a linear combination of the
/// constraints of a [`Problem`](crate::Problem).
///
/// Multiplier `i` applies to constraint number `i` (constraints are numbered in their
/// addition order). Depending on what the solver proved, the multipliers are:
///
/// - a *Farkas certificate of infeasibility*: the linear combination of the constraints
///   contradicts the variables' bounds, proving that the problem admits no solution.
///   Validated by [`Problem::is_infeasibility_certificate_valid`](crate::Problem::is_infeasibility_certificate_valid),
///   which the solver also does itself before handing a certificate out;
/// - an *optimality certificate*: the dual multipliers of an optimal basis, proving that
///   the reported optimal objective value cannot be improved. Validated by
///   [`Problem::is_optimality_certificate_valid`](crate::Problem::is_optimality_certificate_valid).
///
/// A certificate is verified by the solver before being handed out (see the `validation`
/// module). The multipliers are available ([`Self::as_multipliers`]) for independent
/// verification, e.g. with exact arithmetic over the constraints of the problem it was
/// produced for (with the bounds it was produced with).
#[derive(Debug, Clone, PartialEq)]
pub struct Certificate {
    multipliers: Vec<f64>,
}

impl Certificate {
    /// Builds a certificate from the multipliers of the linear combination: multiplier
    /// `i` applies to constraint number `i`.
    pub fn new(multipliers: Vec<f64>) -> Self {
        Self { multipliers }
    }

    /// The multipliers of the linear combination: multiplier `i` applies to constraint
    /// number `i`.
    pub fn as_multipliers(&self) -> &[f64] {
        &self.multipliers
    }
}

/// Explain where the infeasibility comes from.
#[derive(Debug, Clone)]
pub enum InfeasibilityReason {
    /// Obvious infeasibility, that can be typically detected at model construction
    IncompatibleBounds(Variable),
    /// Infeasibility was proven by the solver (typically after a number of simplex pivots).
    /// This variant contains a Farkas certificate that can be used to certify the infeasibility.
    Certified(Certificate),
}

/// Denotes the reason why a feasibility check could not be concluded.
#[derive(Debug, Clone)]
pub enum SolverFailure {
    /// Instability detected in floating point operations, preventing the solver to reach a conclusion.
    Unstable,
    /// Solver reached an explicit time limit before reaching a conclusion.
    ///
    /// Can only occur when calling the solver with an explicit deadline.
    Timeout,
}

/// Error denoting the presence of numerical instability.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NumericalErrors;

/// An error explaining why optimality/feasibility could not be reached.
///
/// Internal to the solver, mainly facilitating error propagation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Error {
    /// The problem is infeasible because an added constraint is a contradiction
    /// or because a var has inconsistent bounds: min > max
    IncompatibleBounds(Variable),
    /// Constrains can't simultaneously be satisfied and a certificate was generated
    /// The coefficients of the certificate correspond to the associated constraint
    InfeasibleWithCertificate(Vec<f64>),
    /// The objective function is unbounded.
    Unbounded,
    /// Floating point operations caused unstability
    Unstable,
    Timeout,
}

impl From<NumericalErrors> for Error {
    fn from(_: NumericalErrors) -> Self {
        Self::Unstable
    }
}

impl From<InfeasibilityReason> for Error {
    fn from(value: InfeasibilityReason) -> Self {
        match value {
            InfeasibilityReason::IncompatibleBounds(variable) => Error::IncompatibleBounds(variable),
            InfeasibilityReason::Certified(cert) => Error::InfeasibleWithCertificate(cert.as_multipliers().to_vec()),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::IncompatibleBounds(var) => write!(f, "problem is infeasible: vars with incoherent bounds: {var:?}"),
            Error::InfeasibleWithCertificate(v) => write!(f, "problem is infeasible, certificate: {:?}", v),
            Error::Unbounded => write!(f, "problem is unbounded"),
            Error::Unstable => write!(f, "problem is instable"),
            Error::Timeout => write!(f, "solver did not terminate in the allocated time."),
        }
    }
}

impl std::error::Error for Error {}
