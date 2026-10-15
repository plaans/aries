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

pub mod prelude;
pub mod results;

#[allow(unused_imports)]
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{expr::Constraint, prelude::*};

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

    /// Verify a certificate of unsatisfiability against this LP.
    ///
    /// The certificate is a vector with one coefficient per constraint: each constraint is multiplied
    /// by its coefficient and the results are summed, which must produce a constraint that is
    /// trivially unsatisfiable given the variables' bounds.
    ///
    /// Certificates are typically obtained from [`results::InfeasibilityReason::Certified`] after a failed
    /// [`Lp::solve`] or [`Lp::check_feasibility`] call.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if the number of coefficients does not match the number of constraints.
    pub fn is_certificate_valid(&self, cert: &[f64]) -> bool {
        self.problem.is_certificate_valid(cert)
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
        let solver = match self.get_solver() {
            Ok(solver) => solver,
            Err(infeasibitiy) => {
                return Ok(FeasibilityStatus::Infeasible(infeasibitiy));
            }
        };

        match solver.solve_feasibility() {
            Ok(()) => Ok(FeasibilityStatus::Feasible),
            Err(Error::IncompatibleBounds(var)) => Ok(FeasibilityStatus::Infeasible(
                InfeasibilityReason::IncompatibleBounds(var),
            )),
            Err(Error::InfeasibleWithCertificate(cert)) => {
                Ok(FeasibilityStatus::Infeasible(InfeasibilityReason::Certified(cert)))
            }
            Err(Error::Unstable) => Err(SolverFailure::Unstable),
            Err(Error::Unbounded) => unreachable!("Cannot return unbounded when checking feasibility only"),
        }
    }
    /// Solve the problem, finding an assignment to all variables that maximizes/minimizes the objective.
    ///
    /// Will returns an error if the solve could not terminate and the status of the LP is therefore not known.
    pub fn solve<'this>(&'this mut self) -> Result<SolveStatus<'this>, SolverFailure> {
        let num_vars = self.problem.obj_coeffs.len();
        let direction = self.problem.direction;
        let solver = match self.get_solver() {
            Ok(solver) => solver,
            Err(infeasible) => return Ok(SolveStatus::Infeasible(infeasible)),
        };
        use SolveStatus::*;
        match solver.initial_solve() {
            Ok(()) => Ok(Optimal(Solution {
                num_vars,
                direction,
                solver,
            })),
            Err(Error::IncompatibleBounds(var)) => Ok(Infeasible(InfeasibilityReason::IncompatibleBounds(var))),
            Err(Error::InfeasibleWithCertificate(cert)) => Ok(Infeasible(InfeasibilityReason::Certified(cert))),
            Err(Error::Unstable) => Err(SolverFailure::Unstable),
            Err(Error::Unbounded) => Ok(SolveStatus::Unbounded),
        }
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

/// An error encountered while solving a problem.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
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
}

impl From<results::NumericalErrors> for Error {
    fn from(_: results::NumericalErrors) -> Self {
        Self::Unstable
    }
}

impl From<InfeasibilityReason> for Error {
    fn from(value: InfeasibilityReason) -> Self {
        match value {
            InfeasibilityReason::IncompatibleBounds(variable) => Error::IncompatibleBounds(variable),
            InfeasibilityReason::Certified(items) => Error::InfeasibleWithCertificate(items),
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
        }
    }
}

impl std::error::Error for Error {}

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

    const TOL: f64 = 1e-8;

    /// Calculate the maximum value that the linear sum with the expr coeff can take regarding the bounds of the variables
    ///
    /// Used to verify a certificate of unsatisfiability
    fn calculate_max_expr(&self, expr: &[f64]) -> f64 {
        debug_assert_eq!(expr.len(), self.var_maxs.len());

        expr.iter()
            .enumerate()
            .map(|(i, &coeff)| {
                if coeff.abs() <= Problem::TOL {
                    0.0
                } else if coeff < 0.0 {
                    self.var_mins[i] * coeff
                } else {
                    self.var_maxs[i] * coeff
                }
            })
            .sum()
    }

    /// Calculate the minimum value that the linear sum with the expr coeff can take regarding the bounds of the variables
    ///
    /// Used to verify a certificate of unsatisfiability
    fn calculate_min_expr(&self, expr: &[f64]) -> f64 {
        debug_assert_eq!(expr.len(), self.var_mins.len());

        expr.iter()
            .enumerate()
            .map(|(i, &coeff)| {
                if coeff.abs() <= Problem::TOL {
                    0.0
                } else if coeff > 0.0 {
                    self.var_mins[i] * coeff
                } else {
                    self.var_maxs[i] * coeff
                }
            })
            .sum()
    }

    /// Verify the certificate of unsatisfiability
    ///
    /// Coeffcicient i in cert corresponds to the constraint number i
    /// All constraints are multiplies with their coefficient and summed to construct the unsatisfiable constraint
    ///
    /// Important: the Problem bounds/constraints need to be up to date with the Solver/FeasbilityChecker generating the certificate
    pub fn is_certificate_valid(&self, cert: &[f64]) -> bool {
        debug_assert_eq!(cert.len(), self.constraints.len());

        let mut expr = vec![0.0; self.var_maxs.len()];

        let mut bound = 0.0;

        let mut cmp_op = ComparisonOp::Eq;

        for (const_i, &coef_cert) in cert.iter().enumerate() {
            if coef_cert == 0.0 {
                continue;
            }

            match self.constraints[const_i].1 {
                ComparisonOp::Le => {
                    if coef_cert > 0.0 {
                        cmp_op = ComparisonOp::Le;
                    } else {
                        cmp_op = ComparisonOp::Ge;
                    }
                }
                ComparisonOp::Ge => {
                    if coef_cert > 0.0 {
                        cmp_op = ComparisonOp::Ge;
                    } else {
                        cmp_op = ComparisonOp::Le;
                    }
                }
                ComparisonOp::Eq => {}
            }

            bound += coef_cert * self.constraints[const_i].2;

            for (var_i, coef_var) in self.constraints[const_i].0.iter() {
                expr[var_i] += coef_cert * coef_var;
            }
        }

        match cmp_op {
            ComparisonOp::Ge => {
                let max_expr = self.calculate_max_expr(&expr);
                max_expr < bound //+ Self::TOL
            }
            ComparisonOp::Le => {
                let min_expr = self.calculate_min_expr(&expr);
                min_expr > bound //- Self::TOL
            }
            ComparisonOp::Eq => {
                let min_expr = self.calculate_min_expr(&expr);
                let max_expr = self.calculate_max_expr(&expr);

                // println!("max: {max_expr}, min: {min_expr}, bound: {bound}");

                max_expr < bound /*+ Self::TOL*/ || min_expr > bound //- Self::TOL
            }
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

        test_incremental_constraints(problem, lit_vec, 10, false, true);
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

    fn test_incremental_constraints(
        problem: Problem,
        lit_vec: Vec<Lit>,
        nb_const: usize,
        verbose: bool,
        assert_on: bool,
    ) -> (usize, usize, usize, usize) {
        let mut lp = Lp::from(problem);

        let mut nb_cert_gen_const = 0;
        let mut nb_val_cert_const = 0;

        let mut nb_cert_gen_feas = 0;
        let mut nb_val_cert_feas = 0;

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

            let problem = lp.problem.clone();
            let incremental = {
                let mut lp = lp.clone();
                lp.check_feasibility().unwrap()
            };
            let incremental_solve = { lp.solve().unwrap() };

            use results::FeasibilityStatus::*;
            use results::InfeasibilityReason::*;

            if let Infeasible(Certified(cert)) = &from_scratch {
                let is_certif_valid = problem.is_certificate_valid(&cert[..nb_const]);
                println!("Is certificate valid add_const: {}", is_certif_valid);
                nb_cert_gen_const += 1;
                nb_val_cert_const += is_certif_valid as usize;
            }

            if let SolveStatus::Infeasible(Certified(cert)) = &incremental_solve {
                let is_certif_valid = problem.is_certificate_valid(cert);
                println!("Is certificate valid set_bound: {}", is_certif_valid);
                nb_cert_gen_feas += 1;
                nb_val_cert_feas += is_certif_valid as usize;
            }

            if let Infeasible(Certified(cert)) = &incremental {
                let is_certif_valid = problem.is_certificate_valid(cert);
                println!("Is certificate valid cheak_feas: {}", is_certif_valid);
                nb_cert_gen_feas += 1;
                nb_val_cert_feas += is_certif_valid as usize;
            }

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

        (nb_val_cert_const, nb_cert_gen_const, nb_val_cert_feas, nb_cert_gen_feas)
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
        let mut nb_cert_gen_const = 0;
        let mut nb_val_cert_const = 0;

        let mut nb_cert_gen_feas = 0;
        let mut nb_val_cert_feas = 0;

        for seed in first_seed..last_seed {
            println!("Seed: {seed}");

            let (problem, lit_vec) = gen_problem(nb_var, nb_const, min, max, sparse_proportion, seed);

            let (nb_val_cert_const_tmp, nb_cert_gen_const_tmp, nb_val_cert_feas_tmp, nb_cert_gen_feas_tmp) =
                test_incremental_constraints(problem, lit_vec, nb_const, false, assert_on);
            nb_cert_gen_feas += nb_cert_gen_feas_tmp;
            nb_val_cert_feas += nb_val_cert_feas_tmp;

            nb_cert_gen_const += nb_cert_gen_const_tmp;
            nb_val_cert_const += nb_val_cert_const_tmp;
        }

        println!(
            "Proportion valid certificate feas checker: {}, add const: {}",
            nb_val_cert_feas as f32 / nb_cert_gen_feas as f32,
            nb_val_cert_const as f32 / nb_cert_gen_const as f32,
        );
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

            if let Ok(SolveStatus::Infeasible(InfeasibilityReason::Certified(cert))) = sol {
                assert!(problem.is_certificate_valid(&cert));
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
            if let Ok(SolveStatus::Infeasible(InfeasibilityReason::Certified(cert))) = sol {
                assert!(problem.is_certificate_valid(&cert));
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
            if let Ok(SolveStatus::Infeasible(InfeasibilityReason::Certified(cert))) = sol {
                assert!(problem.is_certificate_valid(&cert));
            } else {
                panic!()
            }
        }
    }
}
