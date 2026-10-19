#![doc = include_str!("../README.md")]
#![warn(missing_debug_implementations, missing_docs)]
#![allow(clippy::needless_range_loop)]

mod expr;
mod helpers;
mod lu;
mod mps;
mod ordering;
mod solver;
mod sparse;
mod validation;

pub mod prelude;
pub mod results;

use std::time::{Duration, Instant};

#[allow(unused_imports)]
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{expr::Constraint, prelude::*, solver::Stats};

/// A Linear Program (LP), that can be use to construct the LP and solve it.
#[derive(Clone, Debug)]
pub struct Lp {
    /// The linear program
    problem: Problem,
    /// If present, a solver for the linear program. Importantly, the LP in `problem` and `solver`
    /// are kept in sync: any variable, constraint, bound restriction is added to both.
    /// The solver is typically constructed lazily on the first check.
    solver: Option<Box<Solver>>,
}

impl Lp {
    /// Creates the LP from a pre-existing [`Problem`]
    pub fn from(problem: Problem) -> Self {
        Self { problem, solver: None }
    }

    /// Initialize an empty LP, specifying whether it is intended for minimization or maximization.
    pub fn new(dir: OptimizationDirection) -> Self {
        Self {
            problem: Problem::new(dir),
            solver: None,
        }
    }

    /// Declare a new variable in the LP, with the given coefficient in the LP's objective expression as
    /// well as its lower and upper bounds `(lb, ub)`.
    pub fn add_var(&mut self, obj_coeff: f64, bounds: (f64, f64)) -> Variable {
        let (lb, ub) = bounds;
        let var = self.problem.add_var(obj_coeff, (lb, ub));
        if let Some(solver) = self.solver.as_mut() {
            // The solver works on the direction-adjusted objective: `Problem::add_var` negates
            // the coefficient for maximization, so the solver must receive the adjusted value
            let obj_coeff = self.problem.get_obj_coeffs()[var.idx()];
            let var2 = solver.add_variable(obj_coeff, lb, ub);
            assert_eq!(var, var2);
        }
        var
    }

    /// Add a constraint (row) to the LP.
    pub fn add_constraint(&mut self, sum: impl Into<LinExpr>, cmp_op: ComparisonOp, val: f64) {
        let constraint = sum.into().into_constraint(cmp_op, val);
        if let Some(solver) = self.solver.as_mut() {
            solver.add_constraint(&constraint);
        }
        self.problem.add_constraint_impl(constraint);
    }

    /// Updates the bound of the variable, returning an error if the bound is obviously incompatible
    /// with the current opposite bound.
    pub fn set_bound(&mut self, var: Variable, bound: Bound, val: f64) -> Result<(), InfeasibilityReason> {
        self.problem.set_bound(var, bound, val);
        if let Some(checker) = self.solver.as_mut() {
            checker.set_bound(var, bound, val)?;
        }
        Ok(())
    }

    /// Returns a solver for this LP, initializing a new one on its first call.
    ///
    /// Might return an error if an obvious inconsistency is detected when initializing the solver.
    fn get_solver(&mut self) -> Result<&mut Solver, InfeasibilityReason> {
        if self.solver.is_none() {
            let solver = Solver::try_new(
                &self.problem.obj_coeffs,
                &self.problem.var_mins,
                &self.problem.var_maxs,
                &self.problem.constraints,
            )?;
            self.solver = Some(Box::new(solver));
            // from this point on, every modification to the problem must be applied to the problem as well
        }
        Ok(self.solver.as_mut().unwrap())
    }

    /// Checks whether the LP is feasible (admits at least one valid solution).
    /// An error is returned if the results could not be established.
    ///
    /// Note that the objective is completely ignored when checking feasibility.
    pub fn check_feasibility(&mut self) -> Result<FeasibilityStatus, SolverFailure> {
        self.check_feasibility_with_deadline(Deadline::None)
    }

    /// Same as [`Self::check_feasibility`] but accepts a [`Deadline`] at which the search will time-out.
    pub fn check_feasibility_with_deadline(&mut self, deadline: Deadline) -> Result<FeasibilityStatus, SolverFailure> {
        let solver = match self.get_solver() {
            Ok(solver) => solver,
            Err(infeasibitiy) => {
                return Ok(FeasibilityStatus::Infeasible(infeasibitiy));
            }
        };

        match solver.solve_feasibility(deadline) {
            Ok(()) => Ok(FeasibilityStatus::Feasible),
            Err(Error::IncompatibleBounds(var)) => Ok(FeasibilityStatus::Infeasible(
                InfeasibilityReason::IncompatibleBounds(var),
            )),
            // The certificate was verified by the solver before being reported
            // (see the `validation` module).
            Err(Error::InfeasibleWithCertificate(cert)) => Ok(FeasibilityStatus::Infeasible(
                InfeasibilityReason::Certified(Certificate::new(cert)),
            )),
            Err(Error::Unstable) => Err(SolverFailure::Unstable),
            Err(Error::Unbounded) => unreachable!("Cannot return unbounded when checking feasibility only"),
            Err(Error::Timeout) => Err(SolverFailure::Timeout),
        }
    }
    /// Solve the problem, finding an assignment to all variables that maximizes/minimizes the objective.
    ///
    /// Will returns an error if the solve could not terminate and the status of the LP is therefore not known.
    pub fn solve<'this>(&'this mut self) -> Result<SolveStatus<'this>, SolverFailure> {
        self.solve_with_deadline(Deadline::None)
    }

    /// Same as [`Self::solve`] but accepts a [`Deadline`] at which the search will time-out.
    pub fn solve_with_deadline<'this>(
        &'this mut self,
        deadline: Deadline,
    ) -> Result<SolveStatus<'this>, SolverFailure> {
        let num_vars = self.problem.obj_coeffs.len();
        let direction = self.problem.direction;
        let solver = match self.get_solver() {
            Ok(solver) => solver,
            Err(infeasible) => return Ok(SolveStatus::Infeasible(infeasible)),
        };
        use SolveStatus::*;
        match solver.initial_solve(deadline) {
            // The solution and its optimality certificate were verified by the solver
            // before being reported (see the `validation` module).
            Ok(dual_multipliers) => Ok(Optimal(Solution {
                num_vars,
                direction,
                optimality_certificate: Certificate::new(dual_multipliers),
                solver,
            })),
            Err(Error::IncompatibleBounds(var)) => Ok(Infeasible(InfeasibilityReason::IncompatibleBounds(var))),
            // The certificate was verified by the solver before being reported.
            Err(Error::InfeasibleWithCertificate(cert)) => {
                Ok(Infeasible(InfeasibilityReason::Certified(Certificate::new(cert))))
            }
            Err(Error::Unstable) => Err(SolverFailure::Unstable),
            Err(Error::Unbounded) => Ok(SolveStatus::Unbounded),
            Err(Error::Timeout) => Err(SolverFailure::Timeout),
        }
    }

    /// Returns solver statistics.
    pub fn solver_stats(&self) -> &Stats {
        self.solver.as_ref().map_or(&Stats::DEFAULT, |s| &s.stats)
    }
}

use results::{FeasibilityStatus, InfeasibilityReason, SolveStatus, SolverFailure};

impl PartialEq for Lp {
    fn eq(&self, other: &Self) -> bool {
        // only the linear programs need to be equal, the checker is just materialized for efficiency
        self.problem == other.problem
    }
}

use solver::Solver;

/// An enum indicating whether to minimize or maximize objective function.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OptimizationDirection {
    /// Minimize the objective function.
    Minimize,
    /// Maximize the objective function.
    Maximize,
}

/// A reference to a variable in a linear programming problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Variable(pub(crate) usize);

impl Variable {
    /// Sequence number of the variable.
    ///
    /// Variables are referenced by their number in the addition sequence. The method returns
    /// this number.
    #[inline(always)]
    pub fn idx(&self) -> usize {
        self.0
    }
}

/// An operator specifying the relation between left-hand and right-hand sides of the constraint.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ComparisonOp {
    /// The == operator (equal to)
    Eq,
    /// The <= operator (less than or equal to)
    Le,
    /// The >= operator (greater than or equal to)
    Ge,
}

/// Absolute deadline to pass to a solver to limit its runtime.
#[derive(Copy, Clone, Debug)]
pub enum Deadline {
    /// Maximum number of pivots SINCE THE SOLVER CREATION.
    ///
    /// The budget is checked before each pivot iteration: a call whose budget is already
    /// exhausted returns [`SolverFailure::Timeout`] without doing any work — in particular
    /// without selecting a pivot, which is what keeps repeated no-budget calls almost
    /// free. The work-detection (whether a pivot is needed at all) is accounted as part
    /// of a pivot iteration, so reaching a conclusion (feasible, infeasible, optimal)
    /// requires one spare pivot of budget, even when the conclusion itself needs no
    /// pivot.
    ///
    /// When the budget is exhausted mid-search, the call times out leaving a consistent
    /// state from which a later call resumes, warm-starting from the pivots already
    /// performed.
    NumPivots(usize),
    /// Absolute time at which the solver should stop.
    ///
    /// See [`Self::duration`] and [`Self::duration_s`] for creating deadlines from durations.
    Date(Instant),
    /// Unlimited runtime.
    None,
}

impl Deadline {
    /// An absolute time ([`Instant]).
    pub fn date(deadline: Instant) -> Self {
        Deadline::Date(deadline)
    }

    /// Convenience method to create a deadline from a duration.
    ///
    /// The deadline will be set to `duration` after the date *at which this function is called*
    /// (as given by [`Instant::now`]).
    pub fn duration(duration: Duration) -> Self {
        Self::date(Instant::now() + duration)
    }

    /// Convenience method to specify a deadline from a duration in seconds.
    pub fn duration_s(seconds: f64) -> Self {
        Self::duration(Duration::from_secs_f64(seconds))
    }
}

/// A specification of a linear programming problem: the objective direction, the variables
/// with their bounds, and the constraints.
///
/// This is the low-level, static representation of an LP. It contains no solver state:
/// use [`Lp`] as the entry point for building and solving an LP (including incremental
/// modifications and feasibility checks). [`Lp::from`] can be used to build an [`Lp`] on top
/// of an already-constructed problem.
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Problem {
    direction: OptimizationDirection,
    obj_coeffs: Vec<f64>,
    var_mins: Vec<f64>,
    var_maxs: Vec<f64>,
    constraints: Vec<(CsVec, ComparisonOp, f64)>,
}

impl std::fmt::Debug for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only printing lengths here because actual data is probably huge.
        f.debug_struct("Problem")
            .field("direction", &self.direction)
            .field("num_vars", &self.obj_coeffs.len())
            .field("num_constraints", &self.constraints.len())
            .finish()
    }
}

type CsVec = sprs::CsVecI<f64, usize>;

impl Problem {
    /// Create a new problem instance.
    pub fn new(direction: OptimizationDirection) -> Self {
        Problem {
            direction,
            obj_coeffs: vec![],
            var_mins: vec![],
            var_maxs: vec![],
            constraints: vec![],
        }
    }

    /// Add a new variable to the problem.
    ///
    /// `obj_coeff` is a coefficient of the term in the objective function corresponding to this
    /// variable, `min` and `max` are the minimum and maximum (inclusive) bounds of this
    /// variable. If one of the bounds is absent, use `f64::NEG_INFINITY` for minimum and
    /// `f64::INFINITY` for maximum.
    pub fn add_var(&mut self, obj_coeff: f64, (min, max): (f64, f64)) -> Variable {
        let var = Variable(self.obj_coeffs.len());
        let obj_coeff = match self.direction {
            OptimizationDirection::Minimize => obj_coeff,
            OptimizationDirection::Maximize => -obj_coeff,
        };
        self.obj_coeffs.push(obj_coeff);
        self.var_mins.push(min);
        self.var_maxs.push(max);
        var
    }

    /// Add a linear constraint to the problem.
    ///
    /// # Panics
    ///
    /// Will panic if a variable was added more than once to the left-hand side expression.
    ///
    /// # Examples
    ///
    /// Left-hand side of the constraint can be specified in several ways:
    /// ```
    /// # use aries_lp::prelude::*;
    /// let mut problem = Problem::new(OptimizationDirection::Minimize);
    /// let x = problem.add_var(1.0, (0.0, f64::INFINITY));
    /// let y = problem.add_var(1.0, (0.0, f64::INFINITY));
    ///
    /// // Add an x + y >= 2 constraint, specifying the left-hand side expression:
    ///
    /// // * by passing a slice of pairs (useful when explicitly enumerating variables)
    /// problem.add_constraint(x + y, ComparisonOp::Ge, 2.0);
    ///
    /// // * by passing an iterator of variable-coefficient pairs.
    /// let vars = [x, y];
    /// problem.add_constraint(vars.iter().map(|&v| (v, 1.0)), ComparisonOp::Ge, 2.0);
    ///
    /// // * by manually constructing a LinearExpr.
    /// let mut lhs = LinExpr::zero();
    /// for &v in &vars {
    ///     lhs += v;
    /// }
    /// problem.add_constraint(lhs, ComparisonOp::Ge, 2.0);
    /// ```
    pub fn add_constraint(&mut self, expr: impl Into<LinExpr>, cmp_op: ComparisonOp, rhs: f64) {
        self.add_constraint_impl(expr.into().into_constraint(cmp_op, rhs))
    }
    fn add_constraint_impl(&mut self, c: Constraint) {
        self.constraints.push(c.decompose(self.obj_coeffs.len()));
    }

    /// Set a new upper/lower bound to an existing variable
    pub fn set_bound(&mut self, var: Variable, bound: Bound, val: f64) {
        debug_assert!(var.0 < self.var_maxs.len());

        match bound {
            Bound::Upper => self.var_maxs[var.0] = val,
            Bound::Lower => self.var_mins[var.0] = val,
        }
    }

    /// Returns the optimization direction
    pub fn get_direction(&self) -> OptimizationDirection {
        self.direction
    }

    /// Returns a reference to the list of objective coefficients
    pub fn get_obj_coeffs(&self) -> &[f64] {
        &self.obj_coeffs
    }

    /// Returns a reference to the list of lower bounds
    pub fn get_var_mins(&self) -> &[f64] {
        &self.var_mins
    }

    /// Returns a reference to the list of upper bounds
    pub fn get_var_maxs(&self) -> &[f64] {
        &self.var_maxs
    }

    /// Returns a reference to the list of constraints
    pub fn get_constraints(&self) -> &[(CsVec, ComparisonOp, f64)] {
        &self.constraints
    }
}

/// Used to select the bound we want to modify when using set_bound
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Bound {
    /// Lower bound
    Lower,
    /// Higher bound
    Upper,
}

/// A solution of a problem: optimal objective function value and variable values.
///
/// Note that a `Solution` instance contains the whole solver machinery which can require
/// a lot of memory for larger problems. Thus saving the `Solution` instance (as opposed
/// to getting the values of interest and discarding the solution) is mainly useful if you
/// want to add more constraints to it later.
#[derive(Clone)]
pub struct Solution<'solver> {
    direction: OptimizationDirection,
    num_vars: usize,
    /// The dual multipliers of the optimal basis: an optimality certificate.
    optimality_certificate: Certificate,
    solver: &'solver solver::Solver,
}

impl<'solver> std::fmt::Debug for Solution<'solver> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only printing lengths here because actual data is probably huge.
        f.debug_struct("Solution")
            .field("direction", &self.direction)
            .field("num_vars", &self.num_vars)
            .field("num_constraints", &self.solver.num_constraints())
            .field("objective", &self.objective())
            .finish()
    }
}

impl<'solver> Solution<'solver> {
    /// Optimal value of the objective function.
    pub fn objective(&self) -> f64 {
        match self.direction {
            OptimizationDirection::Minimize => self.solver.cur_obj_val,
            OptimizationDirection::Maximize => -self.solver.cur_obj_val,
        }
    }

    /// The optimality certificate of this solution: the dual multipliers of the optimal
    /// basis, proving that the reported objective value cannot be improved upon.
    ///
    /// The certificate is verified by the solver before the solution is handed out (see
    /// the `validation` module); the multipliers are available for independent
    /// verification (e.g. with exact arithmetic).
    pub fn optimality_certificate(&self) -> &Certificate {
        &self.optimality_certificate
    }

    /// Value of the variable at optimum.
    ///
    /// Note that you can use indexing operations to get variable values.
    pub fn var_value(&self, var: Variable) -> &f64 {
        assert!(var.0 < self.num_vars);
        self.solver.get_value(var.0)
    }

    /// Iterate over the variable-value pairs of the solution.
    pub fn iter(&self) -> SolutionIter<'_> {
        SolutionIter {
            solution: self,
            var_idx: 0,
        }
    }
}

impl<'a> std::ops::Index<Variable> for Solution<'a> {
    type Output = f64;

    fn index(&self, var: Variable) -> &Self::Output {
        self.var_value(var)
    }
}

/// An iterator over the variable-value pairs of a [`Solution`].
#[derive(Debug, Clone)]
pub struct SolutionIter<'a> {
    solution: &'a Solution<'a>,
    var_idx: usize,
}

impl<'a> Iterator for SolutionIter<'a> {
    type Item = (Variable, &'a f64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.var_idx < self.solution.num_vars {
            let var_idx = self.var_idx;
            self.var_idx += 1;
            Some((Variable(var_idx), self.solution.solver.get_value(var_idx)))
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a Solution<'a> {
    type Item = (Variable, &'a f64);
    type IntoIter = SolutionIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub use mps::MpsFile;

impl From<Bound> for ComparisonOp {
    fn from(bound: Bound) -> ComparisonOp {
        match bound {
            Bound::Upper => ComparisonOp::Le,
            Bound::Lower => ComparisonOp::Ge,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use rand::{Rng, SeedableRng, rngs::SmallRng, seq::IteratorRandom};

    #[test]
    fn optimize() {
        let mut problem = Lp::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(3.0, (12.0, f64::INFINITY));
        let v2 = problem.add_var(4.0, (5.0, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 20.0);
        problem.add_constraint([(v2, -4.0), (v1, 1.0)], ComparisonOp::Ge, -20.0);

        let sol = problem.solve().unwrap();
        let sol = sol.solution().unwrap();
        assert_eq!(sol[v1], 12.0);
        assert_eq!(sol[v2], 8.0);
        assert_eq!(sol.objective(), 68.0);
    }

    #[test]
    fn empty_expr_constraints() {
        let trivial = [
            (LinExpr::zero(), ComparisonOp::Eq, 0.0),
            (LinExpr::zero(), ComparisonOp::Ge, -1.0),
            (LinExpr::zero(), ComparisonOp::Le, 1.0),
        ];

        let mut problem = Lp::new(OptimizationDirection::Minimize);
        let _ = problem.add_var(1.0, (0.0, f64::INFINITY));
        for (expr, op, b) in trivial.iter().cloned() {
            problem.add_constraint(expr, op, b);
        }
        assert!(problem.solve().unwrap().is_optimal_and(|s| s.objective() == 0.0));

        {
            let mut problem = problem.clone();
            for (expr, op, b) in trivial.iter().cloned() {
                problem.add_constraint(expr, op, b);
            }
            let sol = problem.solve().unwrap();
            assert!(sol.is_optimal_and(|s| s.objective() == 0.0));
        }

        let infeasible = [
            (LinExpr::zero(), ComparisonOp::Eq, 12.0),
            (LinExpr::zero(), ComparisonOp::Ge, 34.0),
            (LinExpr::zero(), ComparisonOp::Le, -56.0),
        ];

        for (expr, op, b) in infeasible.iter().cloned() {
            let mut cloned = problem.clone();
            cloned.add_constraint(expr, op, b);
            assert!(cloned.solve().unwrap().infeasible());
        }

        let _ = problem.add_var(-1.0, (0.0, f64::INFINITY));
        assert!(problem.solve().unwrap().unbounded());
    }

    #[test]
    fn free_variables() {
        let mut problem = Lp::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, f64::INFINITY));
        let v2 = problem.add_var(2.0, (f64::NEG_INFINITY, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 2.0);
        problem.add_constraint([(v1, 1.0), (v2, -1.0)], ComparisonOp::Ge, 0.0);

        let sol = problem.solve().unwrap();
        let sol = sol.solution().unwrap();
        assert_eq!(sol[v1], 2.0);
        assert_eq!(sol[v2], 2.0);
        assert_eq!(sol.objective(), 6.0);
    }

    #[test]
    fn add_variable() {
        let mut problem = Lp::new(OptimizationDirection::Minimize);
        let v1 = problem.add_var(2.0, (0.0, f64::INFINITY));
        let v2 = problem.add_var(1.0, (0.0, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 2.0);

        let res = problem.check_feasibility().unwrap();
        assert!(res.feasible());

        let v3 = problem.add_var(2.0, (f64::NEG_INFINITY, f64::INFINITY));
        let v4 = problem.add_var(-3.0, (-10.0, f64::INFINITY));
        let _v5 = problem.add_var(1.0, (f64::NEG_INFINITY, 10.0));
        let v6 = problem.add_var(0.0, (-10.0, 10.0));

        // Adding constraint with the new variables
        problem.add_constraint([(v3, 1.0), (v4, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v6, 1.0)], ComparisonOp::Ge, 10.0);

        let res = problem.set_bound(v6, Bound::Upper, 5.0);
        let res_check = problem.check_feasibility().unwrap();

        assert!(res.is_err() || res_check.infeasible());
    }

    #[test]
    fn set_ub_var() {
        let mut problem = Lp::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, 3.0));
        let v2 = problem.add_var(2.0, (0.0, 3.0));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

        {
            let mut problem = problem.clone();
            problem.set_bound(v1, Bound::Upper, 0.5).unwrap();
            let sol = problem.solve().unwrap().solution().unwrap();
            assert_eq!(sol[v1], 0.5);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 6.5);

            problem.set_bound(v1, Bound::Upper, 3.0).unwrap();
            let sol = problem.solve().unwrap().solution().unwrap();
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);
        }

        {
            let mut problem = problem.clone();
            problem.set_bound(v2, Bound::Upper, 2.5).unwrap();
            let sol = problem.solve().unwrap().solution().unwrap();
            assert_eq!(sol[v1], 1.5);
            assert_eq!(sol[v2], 2.5);
            assert_eq!(sol.objective(), 6.5);

            problem.set_bound(v2, Bound::Upper, 3.0).unwrap();
            let sol = problem.solve().unwrap().solution().unwrap();
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);
        }
    }

    #[test]
    fn set_lb_var() {
        let mut problem = Lp::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, 3.0));
        let v2 = problem.add_var(2.0, (0.0, 3.0));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

        {
            problem.set_bound(v1, Bound::Lower, 1.5).unwrap();
            let sol = problem.solve().unwrap().solution().unwrap();
            assert_eq!(sol[v1], 1.5);
            assert_eq!(sol[v2], 2.5);
            assert_eq!(sol.objective(), 6.5);

            problem.set_bound(v1, Bound::Lower, 0.0).unwrap();
            let sol = problem.solve().unwrap().solution().unwrap();
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);

            assert!(problem.set_bound(v2, Bound::Lower, 4.0).is_err());
        }
    }

    #[test]
    fn check_feasibility() {
        {
            let mut problem = Lp::new(OptimizationDirection::Maximize);
            let v1 = problem.add_var(1.0, (0.0, 3.0));
            let v2 = problem.add_var(2.0, (0.0, 3.0));
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

            assert!(problem.check_feasibility().unwrap().feasible());
        }

        {
            let mut problem = Lp::new(OptimizationDirection::Maximize);
            let v1 = problem.add_var(1.0, (0.0, 3.0));
            let v2 = problem.add_var(2.0, (0.0, 3.0));
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 5.0);

            assert!(problem.check_feasibility().unwrap().infeasible());
        }
    }

    #[test]
    fn set_bound() {
        {
            let mut problem = Lp::new(OptimizationDirection::Maximize);
            let v1 = problem.add_var(1.0, (0.0, 3.0));
            let v2 = problem.add_var(2.0, (0.0, 3.0));
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

            assert!(problem.check_feasibility().unwrap().feasible());

            let r1 = problem.set_bound(v2, Bound::Lower, 2.0);
            let r2 = problem.set_bound(v1, Bound::Lower, 4.0);

            assert!(problem.check_feasibility().unwrap().infeasible() || r1.is_err() || r2.is_err());

            let _ = problem.set_bound(v1, Bound::Lower, 2.0);

            assert!(problem.check_feasibility().unwrap().feasible());
        }
    }

    struct Lit {
        bound: Bound,
        var: Variable,
        val: f64,
    }

    #[test]
    fn fixed_problem() {
        let mut problem = Problem::new(OptimizationDirection::Maximize);

        let x_vars: Vec<Variable> = (0..10).map(|_| problem.add_var(0.0, (0.0, 10.0))).collect();

        let mut lit_vec = Vec::new();

        for i in 0..10 {
            let s = problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY));
            let x = if i < 9 { x_vars[i] } else { x_vars[0] };

            problem.add_constraint([(s, 1.0), (x, -1.0)], ComparisonOp::Eq, 0.0);

            let (bound, val) = if i < 9 {
                if i % 2 == 0 {
                    (Bound::Lower, 2.0)
                } else {
                    (Bound::Upper, 8.0)
                }
            } else {
                // Last constraint intentionally makes x0 infeasible.
                (Bound::Upper, 1.0)
            };

            lit_vec.push(Lit { bound, var: s, val });
        }

        test_incremental_constraints(problem, lit_vec, false, true);
    }

    fn get_nb_x(sparse_proportion: f32, rng: &mut SmallRng, nb_var: usize) -> usize {
        let k = 1.0 / sparse_proportion - 1.0;

        let nb_x_float = (nb_var - 1) as f32 * rng.r#gen::<f32>().powf(k); // we have a f32 in the range [0.0, nb_var - 1)

        1 + nb_x_float as usize
    }

    fn gen_problem(
        nb_var: usize,
        nb_const: usize,
        min: i32,
        max: i32,
        sparse_proportion: f32,
        seed: u64,
    ) -> (Problem, Vec<Lit>) {
        let mut problem = Problem::new(OptimizationDirection::Maximize);

        let mut lit_vec = Vec::new();

        let mut rng = SmallRng::seed_from_u64(seed);

        let vec_var: Vec<Variable> = (0..nb_var)
            .map(|_| problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY)))
            .collect();

        for _ in 0..nb_const {
            let s = problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY));

            let nb_x = get_nb_x(sparse_proportion, &mut rng, nb_var);

            let x_vec: Vec<Variable> = (0..nb_var)
                .choose_multiple(&mut rng, nb_x)
                .iter()
                .map(|&i| vec_var[i])
                .collect();

            let mut expr: Vec<(Variable, f64)> = x_vec.iter().map(|&x| (x, rng.gen_range(min, max) as f64)).collect();

            // println!("Const {i}: {:?}", expr);

            expr.push((s, -1.0));

            problem.add_constraint(expr, ComparisonOp::Eq, 0.0);

            let bound = if rng.gen_bool(0.5) { Bound::Upper } else { Bound::Lower };

            let val = rng.gen_range(min, max) as f64;

            // println!("Bound: {:?}, val: {val}", bound);

            lit_vec.push(Lit { bound, var: s, val });
        }

        (problem, lit_vec)
    }

    fn test_incremental_constraints(problem: Problem, lit_vec: Vec<Lit>, verbose: bool, assert_on: bool) {
        let mut lp = Lp::from(problem);

        for (i, lit) in lit_vec.iter().enumerate() {
            // problem with the added constraint
            // update LP for the incremental checks
            lp.set_bound(lit.var, lit.bound, lit.val).unwrap();

            // from scratch, create a new LP from the problem and solve
            let from_scratch = {
                let problem = lp.problem.clone();
                let mut lp = Lp::from(problem.clone());
                lp.check_feasibility().unwrap()
            };

            let incremental = {
                let mut lp = lp.clone();
                lp.check_feasibility().unwrap()
            };
            let incremental_solve = { lp.solve().unwrap() };

            if from_scratch.infeasible() != incremental.infeasible()
                || from_scratch.infeasible() != incremental_solve.infeasible()
            {
                println!(
                    "{i} INCONSISTENT RESULTS ======\nfrom_scratch: {:?}\nincremental: {:?}\nincremental_solve: {:?}",
                    from_scratch, incremental, incremental_solve
                );
                assert!(!assert_on);
                break;
            }

            if from_scratch.infeasible() || incremental.infeasible() || incremental_solve.infeasible() {
                if verbose {
                    println!(
                        "{i}\nfrom_scratch: {:?}\nincremental: {:?}\nincremental_solve: {:?}",
                        from_scratch, incremental, incremental_solve
                    );
                }
                break;
            }
        }
    }

    #[test]
    fn test_get_nb_x() {
        let mut rng = SmallRng::seed_from_u64(1);

        let n = 1000;

        let mut mean = 0.0;
        for _ in 0..n {
            mean += get_nb_x(0.42, &mut rng, 100) as f64;
        }

        mean /= n as f64;

        println!("Mean: {mean}");
    }

    #[allow(clippy::too_many_arguments)]
    fn rand_problems(
        first_seed: u64,
        last_seed: u64,
        nb_var: usize,
        nb_const: usize,
        min: i32,
        max: i32,
        sparse_proportion: f32,
        assert_on: bool,
    ) {
        for seed in first_seed..last_seed {
            println!("Seed: {seed}");

            let (problem, lit_vec) = gen_problem(nb_var, nb_const, min, max, sparse_proportion, seed);
            test_incremental_constraints(problem, lit_vec, false, assert_on);
        }
    }

    #[test]
    fn rand_tiny_problems() {
        rand_problems(1, 1000, 5, 10, -10, 10, 0.3, true);
    }

    #[ignore]
    #[test]
    fn rand_10_problems() {
        rand_problems(0, 1000, 50, 100, -10, 10, 0.05, true);
    }
    #[ignore]
    #[test]
    fn rand_5k_problems() {
        rand_problems(0, 1000, 50, 100, -5000, 5000, 0.05, true);
    }

    #[ignore]
    #[test]
    fn rand_i32_min_max_problems() {
        rand_problems(0, 1000, 50, 100, i32::MIN, i32::MAX, 0.05, false);
    }

    #[test]
    fn certificate() {
        {
            let mut problem = Lp::new(OptimizationDirection::Maximize);
            let x1 = problem.add_var(0.0, (0.0, f64::INFINITY));
            let x2 = problem.add_var(0.0, (0.0, f64::INFINITY));
            problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Le, 5.0);
            problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 10.0);

            let sol = problem.solve();

            // The certificate was validated by the solver before being reported.
            if let Ok(SolveStatus::Infeasible(InfeasibilityReason::Certified(cert))) = sol {
                assert_eq!(cert.as_multipliers().len(), 2);
            } else {
                panic!()
            }
        }

        {
            let mut problem = Lp::new(OptimizationDirection::Maximize);
            let x1 = problem.add_var(0.0, (0.0, 3.0));
            let x2 = problem.add_var(0.0, (0.0, 4.0));
            problem.add_constraint([(x1, 1.0), (x2, 2.0)], ComparisonOp::Le, 15.0);
            problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 9.0);

            let sol = problem.solve();
            // The certificate was validated by the solver before being reported.
            if let Ok(SolveStatus::Infeasible(InfeasibilityReason::Certified(cert))) = sol {
                assert_eq!(cert.as_multipliers().len(), 2);
            } else {
                panic!()
            }
        }

        {
            let mut problem = Lp::new(OptimizationDirection::Maximize);
            let x1 = problem.add_var(0.0, (0.0, 2.0));
            let x2 = problem.add_var(0.0, (0.0, 2.0));
            problem.add_constraint([(x1, 2.0), (x2, 1.0)], ComparisonOp::Ge, 5.0);
            problem.add_constraint([(x1, -1.0), (x2, 1.0)], ComparisonOp::Ge, 2.0);

            let sol = problem.solve();
            // The certificate was validated by the solver before being reported.
            if let Ok(SolveStatus::Infeasible(InfeasibilityReason::Certified(cert))) = sol {
                assert_eq!(cert.as_multipliers().len(), 2);
            } else {
                panic!()
            }
        }
    }

    #[test]
    fn feasibility_check_preserves_exact_optimum() {
        // Feasibility restoration perturbs the objective coefficients to break ties in the
        // dual ratio test; a subsequent solve must discard them entirely and report the
        // exact optimum of the true objective.
        let mut problem = Lp::new(OptimizationDirection::Minimize);
        let x1 = problem.add_var(1.0, (0.0, 1.0));
        let x2 = problem.add_var(1.0, (0.0, 1.0));
        problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 2.0);

        assert!(problem.check_feasibility().unwrap().feasible());

        let sol = problem.solve().unwrap().solution().unwrap();
        assert_eq!(sol[x1] + sol[x2], 2.0);
        assert_eq!(sol.objective(), 2.0);
    }

    #[test]
    fn infeasibility_certificate_after_bound_flips() {
        // x1 + x2 >= 3 with both vars in [0, 1]: only after flipping both vars to their
        // upper bound does the ratio test prove that the constraint cannot be satisfied.
        let mut problem = Lp::new(OptimizationDirection::Maximize);
        let x1 = problem.add_var(0.0, (0.0, 1.0));
        let x2 = problem.add_var(0.0, (0.0, 1.0));
        problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 3.0);

        // The certificate was validated by the solver before being reported.
        match problem.check_feasibility().unwrap() {
            FeasibilityStatus::Infeasible(InfeasibilityReason::Certified(cert)) => {
                assert_eq!(cert.as_multipliers().len(), 1);
            }
            status => panic!("expected a certified infeasibility, got {status:?}"),
        }
    }

    /// Builds a degenerate feasibility-only LP (zero objective, duplicated equality
    /// rows) together with the order in which its variables must be fixed to a hidden
    /// satisfying assignment — the usage pattern of the CP solver's LP reasoner.
    fn degenerate_problem_and_fixing_order(seed: u64) -> (Lp, Vec<Variable>, Vec<usize>) {
        let mut rng = SmallRng::seed_from_u64(seed);
        let num_vars = 10 + rng.gen_range(0, 10);
        let num_cons = num_vars * 2 + rng.gen_range(0, num_vars);
        const HIDDEN: f64 = 2.0; // the assignment all vars will be fixed to

        let mut problem = Lp::new(OptimizationDirection::Minimize);
        let vars: Vec<Variable> = (0..num_vars).map(|_| problem.add_var(0.0, (0.0, 4.0))).collect();

        // Equalities satisfied by the hidden assignment; duplicated rows make the
        // instance highly degenerate (many rows are linear combinations of others).
        for _ in 0..num_cons {
            let nnz = 2 + rng.gen_range(0, 3);
            let mut terms: Vec<(Variable, f64)> = vec![];
            for _ in 0..nnz {
                let v = vars[rng.gen_range(0, num_vars)];
                let c = 1.0 + rng.gen_range(0, 2) as f64;
                match terms.iter_mut().find(|(t, _)| *t == v) {
                    Some(entry) => entry.1 += c,
                    None => terms.push((v, c)),
                }
            }
            terms.sort_by_key(|(v, _)| v.idx());
            let rhs = HIDDEN * terms.iter().map(|(_, c)| *c).sum::<f64>();
            problem.add_constraint(terms, ComparisonOp::Eq, rhs);
        }

        let mut order: Vec<usize> = (0..num_vars).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.gen_range(0, i + 1));
        }
        (problem, vars, order)
    }

    #[test]
    fn optimality_certificates_are_validated() {
        // A maximization problem with optimum 68 at (12, 8). The solver verifies the
        // solution and its optimality certificate before reporting them (see the
        // `validation` module): solving successfully is the validation passing.
        let mut problem = Lp::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(3.0, (0.0, 12.0));
        let v2 = problem.add_var(4.0, (0.0, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 20.0);
        problem.add_constraint([(v2, -4.0), (v1, 1.0)], ComparisonOp::Ge, -20.0);

        let solution = problem.solve().unwrap().solution().unwrap();
        assert_eq!(solution.objective(), 68.0);

        // The solution carries the verified optimality certificate: the dual
        // multipliers, one per constraint, available for independent verification.
        let cert = solution.optimality_certificate();
        assert_eq!(cert.as_multipliers().len(), 2);
    }

    #[test]
    fn degenerate_feasibility_checks_conclude() {
        // Feasibility-only usage (zero objective) on highly degenerate instances, with
        // variables progressively fixed to a hidden satisfying assignment — the pattern
        // of the CP solver's LP reasoner, and the worst case for the dual simplex: all
        // reduced costs are zero at all times, so without anti-cycling measures the dual
        // ratio test is fully degenerate and can cycle. Every check must conclude within
        // a generous pivot budget.
        for seed in 1..=20u64 {
            let (mut problem, vars, order) = degenerate_problem_and_fixing_order(seed);
            for &v in &order {
                problem.set_bound(vars[v], Bound::Lower, 2.0).unwrap();
                problem.set_bound(vars[v], Bound::Upper, 2.0).unwrap();
                let res = problem
                    .check_feasibility_with_deadline(Deadline::NumPivots(10_000))
                    .unwrap();
                assert!(res.feasible(), "seed {seed}: fixing var {v} must stay feasible");
            }
        }
    }

    #[test]
    fn feasibility_checks_conclude_under_tight_pivot_budgets() {
        // Mimics the LP reasoner's default rhythm: the pivot budget starts at zero and
        // grows by a fraction of a pivot per decision, so each check performs at most a
        // handful of pivots — sometimes none — and the restoration of feasibility is
        // spread over many calls. Interrupted checks must leave a consistent resumable
        // state, and the final verdict must be correct.
        for budget_increment in [0usize, 1, 3] {
            for seed in 1..=10u64 {
                let (mut problem, vars, order) = degenerate_problem_and_fixing_order(seed);
                for &v in &order {
                    problem.set_bound(vars[v], Bound::Lower, 2.0).unwrap();
                    problem.set_bound(vars[v], Bound::Upper, 2.0).unwrap();

                    // Whatever budget is left, plus a small increment.
                    let max_pivots = problem.solver_stats().num_pivots + budget_increment;
                    match problem.check_feasibility_with_deadline(Deadline::NumPivots(max_pivots)) {
                        Ok(FeasibilityStatus::Feasible) | Err(SolverFailure::Timeout) => {}
                        other => panic!("seed {seed}, budget +{budget_increment}: unexpected result {other:?}"),
                    }
                }
                // Once the search is over, a check with a generous budget must conclude.
                let res = problem
                    .check_feasibility_with_deadline(Deadline::NumPivots(10_000))
                    .unwrap();
                assert!(
                    res.feasible(),
                    "seed {seed}, budget +{budget_increment}: must be feasible in the end"
                );
                assert!(problem.solver_stats().num_pivots < 10_000);
            }
        }
    }
}
