#![allow(clippy::needless_range_loop)]

/*!
A fast linear programming solver library.

[Linear programming](https://en.wikipedia.org/wiki/Linear_programming) is a technique for
finding the minimum (or maximum) of a linear function of a set of continuous variables
subject to linear equality and inequality constraints.

# Features

* Pure Rust implementation.
* Able to solve problems with hundreds of thousands of variables and constraints.
* Incremental: add constraints to an existing solution without solving it from scratch.
* Problems can be defined via an API or parsed from an
  [MPS](https://en.wikipedia.org/wiki/MPS_(format)) file.

# Entry points

Begin by creating a [`Problem`](struct.Problem.html) instance, declaring variables and adding
constraints. Solving it will produce a [`Solution`](struct.Solution.html) that can be used to
get the optimal objective value, corresponding variable values and to add more constraints
to the problem.

Alternatively, create an [`MpsFile`](mps/struct.MpsFile.html) by parsing a file in the MPS format.

# Example

```
use minilp::{Problem, OptimizationDirection, ComparisonOp};

// Maximize an objective function x + 2 * y of two variables x >= 0 and 0 <= y <= 3
let mut problem = Problem::new(OptimizationDirection::Maximize);
let x = problem.add_var(1.0, (0.0, f64::INFINITY));
let y = problem.add_var(2.0, (0.0, 3.0));

// subject to constraints: x + y <= 4 and 2 * x + y >= 2.
problem.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Le, 4.0);
problem.add_constraint([(x, 2.0), (y, 1.0)], ComparisonOp::Ge, 2.0);

// Optimal value is 7, achieved at x = 1 and y = 3.
let solution = problem.solve().unwrap();
assert_eq!(solution.objective(), 7.0);
assert_eq!(solution[x], 1.0);
assert_eq!(solution[y], 3.0);
```
*/

#![deny(missing_debug_implementations, missing_docs)]

#[macro_use]
extern crate log;

mod helpers;
mod lu;
mod mps;
mod ordering;
mod solver;
mod sparse;

use serde::{Deserialize, Serialize};
use solver::Solver;

/// An enum indicating whether to minimize or maximize objective function.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum OptimizationDirection {
    /// Minimize the objective function.
    Minimize,
    /// Maximize the objective function.
    Maximize,
}

/// A reference to a variable in a linear programming problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Variable(pub(crate) usize);

impl Variable {
    /// Sequence number of the variable.
    ///
    /// Variables are referenced by their number in the addition sequence. The method returns
    /// this number.
    pub fn idx(&self) -> usize {
        self.0
    }
}

/// A sum of variables multiplied by constant coefficients used as a left-hand side
/// when defining constraints.
#[derive(Clone, Debug)]
pub struct LinearExpr {
    vars: Vec<usize>,
    coeffs: Vec<f64>,
}

impl LinearExpr {
    /// Creates an empty linear expression.
    pub fn empty() -> Self {
        Self {
            vars: vec![],
            coeffs: vec![],
        }
    }

    /// Add a single term to the linear expression.
    ///
    /// Variables can be added to an expression in any order, but adding the same variable
    /// several times is forbidden (the [`Problem::add_constraint`] method will panic).
    ///
    /// [`Problem::add_constraint`]: struct.Problem.html#method.add_constraint
    pub fn add(&mut self, var: Variable, coeff: f64) {
        self.vars.push(var.0);
        self.coeffs.push(coeff);
    }
}

/// A single `variable * constant` term in a linear expression.
/// This is an auxiliary struct for specifying conversions.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct LinearTerm(Variable, f64);

impl From<(Variable, f64)> for LinearTerm {
    fn from(term: (Variable, f64)) -> Self {
        LinearTerm(term.0, term.1)
    }
}

impl<'a> From<&'a (Variable, f64)> for LinearTerm {
    fn from(term: &'a (Variable, f64)) -> Self {
        LinearTerm(term.0, term.1)
    }
}

impl<I: IntoIterator<Item = impl Into<LinearTerm>>> From<I> for LinearExpr {
    fn from(iter: I) -> Self {
        let mut expr = LinearExpr::empty();
        for term in iter {
            let LinearTerm(var, coeff) = term.into();
            expr.add(var, coeff);
        }
        expr
    }
}

impl std::iter::FromIterator<(Variable, f64)> for LinearExpr {
    fn from_iter<I: IntoIterator<Item = (Variable, f64)>>(iter: I) -> Self {
        let mut expr = LinearExpr::empty();
        for term in iter {
            expr.add(term.0, term.1)
        }
        expr
    }
}

impl std::iter::Extend<(Variable, f64)> for LinearExpr {
    fn extend<I: IntoIterator<Item = (Variable, f64)>>(&mut self, iter: I) {
        for term in iter {
            self.add(term.0, term.1)
        }
    }
}

/// An operator specifying the relation between left-hand and right-hand sides of the constraint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Constrains can't simultaneously be satisfied.
    Infeasible,
    /// Constrains can't simultaneously be satisfied and a certificate was generated
    InfeasibleWithCertificate(Vec<f64>),
    /// The objective function is unbounded.
    Unbounded,
    /// Floating point operations caused instability
    Instable,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = match self {
            Error::Infeasible => "problem is infeasible",
            Error::InfeasibleWithCertificate(v) => &format!("problem is infeasible, certificate: {:?}", v),
            Error::Unbounded => "problem is unbounded",
            Error::Instable => "problem is instable",
        };
        msg.fmt(f)
    }
}

impl std::error::Error for Error {}

/// A specification of a linear programming problem.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
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
    /// # use minilp::*;
    /// let mut problem = Problem::new(OptimizationDirection::Minimize);
    /// let x = problem.add_var(1.0, (0.0, f64::INFINITY));
    /// let y = problem.add_var(1.0, (0.0, f64::INFINITY));
    ///
    /// // Add an x + y >= 2 constraint, specifying the left-hand side expression:
    ///
    /// // * by passing a slice of pairs (useful when explicitly enumerating variables)
    /// problem.add_constraint([(x, 1.0), (y, 1.0)], ComparisonOp::Ge, 2.0);
    ///
    /// // * by passing an iterator of variable-coefficient pairs.
    /// let vars = [x, y];
    /// problem.add_constraint(vars.iter().map(|&v| (v, 1.0)), ComparisonOp::Ge, 2.0);
    ///
    /// // * by manually constructing a LinearExpr.
    /// let mut lhs = LinearExpr::empty();
    /// for &v in &vars {
    ///     lhs.add(v, 1.0);
    /// }
    /// problem.add_constraint(lhs, ComparisonOp::Ge, 2.0);
    /// ```
    pub fn add_constraint(&mut self, expr: impl Into<LinearExpr>, cmp_op: ComparisonOp, rhs: f64) {
        let expr = expr.into();
        self.constraints
            .push((CsVec::new(self.obj_coeffs.len(), expr.vars, expr.coeffs), cmp_op, rhs));
    }

    /// Solve the problem, finding the optimal objective function value and variable values.
    ///
    /// # Errors
    ///
    /// Will return an error, if the problem is infeasible (constraints can't be satisfied)
    /// or if the objective value is unbounded.
    pub fn solve(&self) -> Result<Solution, Error> {
        let mut solver = Solver::try_new(&self.obj_coeffs, &self.var_mins, &self.var_maxs, &self.constraints)?;
        solver.initial_solve()?;
        Ok(Solution {
            num_vars: self.obj_coeffs.len(),
            direction: self.direction,
            solver,
        })
    }

    /// Create a solver for the problem and wrap it in a FeasbilityChecker
    ///
    /// # Errors
    ///
    /// Will return an error, if the problem is trivially infeasible:
    /// - min > max for a the domain of a var
    /// - contradiction in a constraint free of any var: 0.0 <= -1.0 for example
    pub fn create_feasibility_checker(&self) -> Result<FeasibilityChecker, Error> {
        let solver = Solver::try_new(&self.obj_coeffs, &self.var_mins, &self.var_maxs, &self.constraints)?;
        Ok(FeasibilityChecker { solver })
    }

    /// Set a new upper/lower bound to an existing variable
    pub fn set_bound(&mut self, var: Variable, bound: &Bound, val: f64) {
        debug_assert!(var.0 < self.var_maxs.len());

        match bound {
            Bound::Upper => self.var_maxs[var.0] = val,
            Bound::Lower => self.var_mins[var.0] = val,
        }
    }

    const TOL: f64 = 1e-8;

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

        let res = match cmp_op {
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
        };
        // assert!(res);
        res
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

/// A solution of a problem: optimal objective function value and variable values.
///
/// Note that a `Solution` instance contains the whole solver machinery which can require
/// a lot of memory for larger problems. Thus saving the `Solution` instance (as opposed
/// to getting the values of interest and discarding the solution) is mainly useful if you
/// want to add more constraints to it later.
#[derive(Clone)]
pub struct Solution {
    direction: OptimizationDirection,
    num_vars: usize,
    solver: solver::Solver,
}

impl std::fmt::Debug for Solution {
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

impl Solution {
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

    /// Add another constraint and return the solution to the updated problem.
    ///
    /// This method will consume the solution and not return it in case of error. See also
    /// examples of specifying the left-hand side in the docs for the [`Problem::add_constraint`]
    /// method.
    ///
    /// [`Problem::add_constraint`]: struct.Problem.html#method.add_constraint
    ///
    /// # Errors
    ///
    /// Will return an error if the problem becomes infeasible with the additional constraint.
    pub fn add_constraint(
        mut self,
        expr: impl Into<LinearExpr>,
        cmp_op: ComparisonOp,
        rhs: f64,
    ) -> Result<Self, Error> {
        let expr = expr.into();
        self.solver
            .add_constraint(CsVec::new(self.num_vars, expr.vars, expr.coeffs), cmp_op, rhs)?;
        Ok(self)
    }

    /// Fix the variable to the specified value and return the solution to the updated problem.
    ///
    /// This method will consume the solution and not return it in case of error.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem becomes infeasible with the additional constraint.
    pub fn fix_var(mut self, var: Variable, val: f64) -> Result<Self, Error> {
        assert!(var.0 < self.num_vars);
        self.solver.fix_var(var.0, val)?;
        Ok(self)
    }

    /// Fix the upper bound of a variable to the specified value and return the solution to the updated problem.
    ///
    /// This method will consume the solution and not return it in case of error.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem becomes infeasible with the additional constraint.
    pub fn set_ub_var(mut self, var: Variable, val: f64) -> Result<Self, Error> {
        assert!(var.0 < self.num_vars);
        self.solver.set_ub_var(var.0, val)?;
        self.solver.initial_solve()?;
        Ok(self)
    }

    /// Fix the upper bound of a variable to the specified value and return the solution to the updated problem.
    ///
    /// This method will consume the solution and not return it in case of error.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem becomes infeasible with the additional constraint.
    pub fn set_lb_var(mut self, var: Variable, val: f64) -> Result<Self, Error> {
        assert!(var.0 < self.num_vars);
        self.solver.set_lb_var(var.0, val)?;
        self.solver.initial_solve()?;
        Ok(self)
    }

    /// If the variable was fixed with [`fix_var`](#method.fix_var) before, remove that constraint
    /// and return the solution to the updated problem and a boolean indicating if the variable was
    /// really fixed before.
    pub fn unfix_var(mut self, var: Variable) -> (Self, bool) {
        assert!(var.0 < self.num_vars);
        let res = self.solver.unfix_var(var.0);
        (self, res)
    }

    // TODO: remove_constraint

    /// Add a [Gomory cut] constraint to the problem and return the solution.
    ///
    /// [Gomory cut]: https://en.wikipedia.org/wiki/Cutting-plane_method#Gomory's_cut
    ///
    /// # Errors
    ///
    /// Will return an error if the problem becomes infeasible with the additional constraint.
    ///
    /// # Panics
    ///
    /// Will panic if the variable is not basic (variable is basic if it has value other than
    /// its bounds).
    pub fn add_gomory_cut(mut self, var: Variable) -> Result<Self, Error> {
        assert!(var.0 < self.num_vars);
        self.solver.add_gomory_cut(var.0)?;
        Ok(self)
    }
}

impl std::ops::Index<Variable> for Solution {
    type Output = f64;

    fn index(&self, var: Variable) -> &Self::Output {
        self.var_value(var)
    }
}

/// An iterator over the variable-value pairs of a [`Solution`].
#[derive(Debug, Clone)]
pub struct SolutionIter<'a> {
    solution: &'a Solution,
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

impl<'a> IntoIterator for &'a Solution {
    type Item = (Variable, &'a f64);
    type IntoIter = SolutionIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub use mps::MpsFile;

/// Used to select the bound we want to modify when using set_bound
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Bound {
    /// Lower bound
    Lower,
    /// Higher bound
    Upper,
}

impl From<Bound> for ComparisonOp {
    fn from(bound: Bound) -> ComparisonOp {
        match bound {
            Bound::Upper => ComparisonOp::Le,
            Bound::Lower => ComparisonOp::Ge,
        }
    }
}

/// Allow us to check the faisability of a problem and modify the bounds of its variables
#[derive(Debug, Clone)]
pub struct FeasibilityChecker {
    solver: Solver,
}

impl FeasibilityChecker {
    /// Set a new Upper/Lower bound for the given variable and return the previous bound
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediatly detected as infeasible.
    pub fn set_bound(&mut self, var: Variable, bound: &Bound, val: f64) -> Result<f64, Error> {
        let old_val_solver = match bound {
            Bound::Lower => self.solver.set_lb_var(var.0, val)?,
            Bound::Upper => self.solver.set_ub_var(var.0, val)?,
        };

        Ok(old_val_solver)
    }

    /// Add a new constraint to our lp
    /// The variables involved must be already declared
    ///
    /// [`Problem::add_constraint`]: struct.Problem.html#method.add_constraint
    ///
    /// # Errors
    ///
    /// Will return an error if the problem becomes infeasible with the additional constraint.
    pub fn add_constraint(&mut self, expr: impl Into<LinearExpr>, cmp_op: ComparisonOp, rhs: f64) -> Result<(), Error> {
        let expr = expr.into();
        self.solver
            .add_constraint(CsVec::new(self.solver.num_vars, expr.vars, expr.coeffs), cmp_op, rhs)
    }

    /// Add a new variable
    ///
    /// # Errors
    ///
    /// Will return an error if the variable has inconsistent bounds
    pub fn add_variable(&mut self, obj_coeff: f64, min: f64, max: f64) -> Result<usize, Error> {
        self.solver.add_variable(obj_coeff, min, max)
    }

    /// Try to restore the feasibility of our problem
    ///
    ///  # Errors
    ///
    /// Will return an error if th feasibility can't be restored
    /// Note that if set_bound already returned an error, cheack_feasibility might not return that the problem is infeasible, both need to be checked
    pub fn check_feasibility(&mut self) -> Result<(), Error> {
        self.solver.solve_feasibility()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use rand::{Rng, SeedableRng, rngs::SmallRng, seq::IteratorRandom};

    #[test]
    fn optimize() {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(3.0, (12.0, f64::INFINITY));
        let v2 = problem.add_var(4.0, (5.0, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 20.0);
        problem.add_constraint([(v2, -4.0), (v1, 1.0)], ComparisonOp::Ge, -20.0);

        let sol = problem.solve().unwrap();
        assert_eq!(sol[v1], 12.0);
        assert_eq!(sol[v2], 8.0);
        assert_eq!(sol.objective(), 68.0);
    }

    #[test]
    fn empty_expr_constraints() {
        let trivial = [
            (LinearExpr::empty(), ComparisonOp::Eq, 0.0),
            (LinearExpr::empty(), ComparisonOp::Ge, -1.0),
            (LinearExpr::empty(), ComparisonOp::Le, 1.0),
        ];

        let mut problem = Problem::new(OptimizationDirection::Minimize);
        let _ = problem.add_var(1.0, (0.0, f64::INFINITY));
        for (expr, op, b) in trivial.iter().cloned() {
            problem.add_constraint(expr, op, b);
        }
        assert_eq!(problem.solve().map(|s| s.objective()), Ok(0.0));

        {
            let mut sol = problem.solve().unwrap();
            for (expr, op, b) in trivial.iter().cloned() {
                sol = sol.add_constraint(expr, op, b).unwrap();
            }
            assert_eq!(sol.objective(), 0.0);
        }

        let infeasible = [
            (LinearExpr::empty(), ComparisonOp::Eq, 12.0),
            (LinearExpr::empty(), ComparisonOp::Ge, 34.0),
            (LinearExpr::empty(), ComparisonOp::Le, -56.0),
        ];

        for (expr, op, b) in infeasible.iter().cloned() {
            let mut cloned = problem.clone();
            cloned.add_constraint(expr, op, b);
            assert_eq!(cloned.solve().map(|_| "solved"), Err(Error::Infeasible));
        }

        for (expr, op, b) in infeasible.iter().cloned() {
            let sol = problem.solve().unwrap().add_constraint(expr, op, b);
            assert_eq!(sol.map(|_| "solved"), Err(Error::Infeasible));
        }

        let _ = problem.add_var(-1.0, (0.0, f64::INFINITY));
        assert_eq!(problem.solve().map(|_| "solved"), Err(Error::Unbounded));
    }

    #[test]
    fn free_variables() {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, f64::INFINITY));
        let v2 = problem.add_var(2.0, (f64::NEG_INFINITY, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 2.0);
        problem.add_constraint([(v1, 1.0), (v2, -1.0)], ComparisonOp::Ge, 0.0);

        let sol = problem.solve().unwrap();
        assert_eq!(sol[v1], 2.0);
        assert_eq!(sol[v2], 2.0);
        assert_eq!(sol.objective(), 6.0);
    }

    #[test]
    fn fix_unfix_var() {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, 3.0));
        let v2 = problem.add_var(2.0, (0.0, 3.0));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

        let orig_sol = problem.solve().unwrap();

        {
            let mut sol = orig_sol.clone().fix_var(v1, 0.5).unwrap();
            assert_eq!(sol[v1], 0.5);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 6.5);

            sol = sol.unfix_var(v1).0;
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);
        }

        {
            let mut sol = orig_sol.clone().fix_var(v2, 2.5).unwrap();
            assert_eq!(sol[v1], 1.5);
            assert_eq!(sol[v2], 2.5);
            assert_eq!(sol.objective(), 6.5);

            sol = sol.unfix_var(v2).0;
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);
        }
    }

    #[test]
    fn add_constraint() {
        let mut problem = Problem::new(OptimizationDirection::Minimize);
        let v1 = problem.add_var(2.0, (0.0, f64::INFINITY));
        let v2 = problem.add_var(1.0, (0.0, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 2.0);

        let orig_sol = problem.solve().unwrap();

        {
            let sol = orig_sol
                .clone()
                .add_constraint([(v1, -1.0), (v2, 1.0)], ComparisonOp::Le, 0.0)
                .unwrap();

            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 1.0);
            assert_eq!(sol.objective(), 3.0);
        }

        {
            let sol = orig_sol
                .clone()
                .fix_var(v2, 1.5)
                .unwrap()
                .add_constraint([(v1, -1.0), (v2, 1.0)], ComparisonOp::Le, 0.0)
                .unwrap();
            assert_eq!(sol[v1], 1.5);
            assert_eq!(sol[v2], 1.5);
            assert_eq!(sol.objective(), 4.5);
        }

        {
            let sol = orig_sol
                .clone()
                .add_constraint([(v1, -1.0), (v2, 1.0)], ComparisonOp::Ge, 3.0)
                .unwrap();

            assert_eq!(sol[v1], 0.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 3.0);
        }
    }

    #[test]
    fn add_variable() {
        let mut problem = Problem::new(OptimizationDirection::Minimize);
        let v1 = problem.add_var(2.0, (0.0, f64::INFINITY));
        let v2 = problem.add_var(1.0, (0.0, f64::INFINITY));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 2.0);

        let mut feas_check = problem.create_feasibility_checker().unwrap();

        let res = feas_check.check_feasibility();
        assert!(res.is_ok());

        let res = feas_check.add_variable(2.0, f64::NEG_INFINITY, f64::INFINITY);
        assert!(res.is_ok());
        let v3 = Variable(res.unwrap());
        assert_eq!(v3, problem.add_var(2.0, (f64::NEG_INFINITY, f64::INFINITY)));

        let res = feas_check.add_variable(-3.0, -10.0, f64::INFINITY);
        assert!(res.is_ok());
        let v4 = Variable(res.unwrap());
        assert_eq!(v4, problem.add_var(-3.0, (-10.0, f64::INFINITY)));

        let res = feas_check.add_variable(1.0, f64::NEG_INFINITY, 10.0);
        assert!(res.is_ok());
        let v5 = Variable(res.unwrap());
        assert_eq!(v5, problem.add_var(1.0, (f64::NEG_INFINITY, 10.0)));

        let res = feas_check.add_variable(0.0, 2.0, 0.0);
        assert!(res.is_err());

        let res = feas_check.add_variable(0.0, -10.0, 10.0);
        assert!(res.is_ok());
        let v6 = Variable(res.unwrap());
        assert_eq!(v6, problem.add_var(0.0, (-10.0, 10.0)));

        // Adding constraint with the new variables
        let res = feas_check.add_constraint([(v3, 1.0), (v4, 1.0)], ComparisonOp::Le, 4.0);
        assert!(res.is_ok());

        let res = feas_check.add_constraint([(v1, 1.0), (v6, 1.0)], ComparisonOp::Ge, 10.0);
        assert!(res.is_ok());

        let res = feas_check.set_bound(v6, &Bound::Upper, 5.0);
        let res_check = feas_check.check_feasibility();

        assert!(res.is_err() || res_check.is_err());
    }

    #[test]
    fn gomory_cut() {
        let mut problem = Problem::new(OptimizationDirection::Minimize);
        let v1 = problem.add_var(0.0, (0.0, f64::INFINITY));
        let v2 = problem.add_var(-1.0, (0.0, f64::INFINITY));
        problem.add_constraint([(v1, 3.0), (v2, 2.0)], ComparisonOp::Le, 6.0);
        problem.add_constraint([(v1, -3.0), (v2, 2.0)], ComparisonOp::Le, 0.0);

        let mut sol = problem.solve().unwrap();
        assert_eq!(sol[v1], 1.0);
        assert_eq!(sol[v2], 1.5);
        assert_eq!(sol.objective(), -1.5);

        sol = sol.add_gomory_cut(v2).unwrap();
        assert!(f64::abs(sol[v1] - 2.0 / 3.0) < 1e-8);
        assert_eq!(sol[v2], 1.0);
        assert_eq!(sol.objective(), -1.0);

        sol = sol.add_gomory_cut(v1).unwrap();
        assert!(f64::abs(sol[v1] - 1.0) < 1e-8);
        assert_eq!(sol[v2], 1.0);
        assert_eq!(sol.objective(), -1.0);
    }

    #[test]
    fn set_ub_var() {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, 3.0));
        let v2 = problem.add_var(2.0, (0.0, 3.0));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

        let orig_sol = problem.solve().unwrap();

        {
            let mut sol = orig_sol.clone().set_ub_var(v1, 0.5).unwrap();
            assert_eq!(sol[v1], 0.5);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 6.5);

            sol = sol.set_ub_var(v1, 3.0).unwrap();
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);
        }

        {
            let mut sol = orig_sol.clone().set_ub_var(v2, 2.5).unwrap();
            assert_eq!(sol[v1], 1.5);
            assert_eq!(sol[v2], 2.5);
            assert_eq!(sol.objective(), 6.5);

            sol = sol.set_ub_var(v2, 3.0).unwrap();
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);
        }
    }

    #[test]
    fn set_lb_var() {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let v1 = problem.add_var(1.0, (0.0, 3.0));
        let v2 = problem.add_var(2.0, (0.0, 3.0));
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
        problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

        let orig_sol = problem.solve().unwrap();

        {
            let mut sol = orig_sol.clone().set_lb_var(v1, 1.5).unwrap();
            assert_eq!(sol[v1], 1.5);
            assert_eq!(sol[v2], 2.5);
            assert_eq!(sol.objective(), 6.5);

            sol = sol.set_lb_var(v1, 0.0).unwrap();
            assert_eq!(sol[v1], 1.0);
            assert_eq!(sol[v2], 3.0);
            assert_eq!(sol.objective(), 7.0);

            assert!(sol.set_lb_var(v2, 4.0).is_err());
        }
    }

    #[test]
    fn check_feasability() {
        {
            let mut problem = Problem::new(OptimizationDirection::Maximize);
            let v1 = problem.add_var(1.0, (0.0, 3.0));
            let v2 = problem.add_var(2.0, (0.0, 3.0));
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

            let mut feas_checker = problem.create_feasibility_checker().unwrap();

            assert!(feas_checker.check_feasibility().is_ok());
        }

        {
            let mut problem = Problem::new(OptimizationDirection::Maximize);
            let v1 = problem.add_var(1.0, (0.0, 3.0));
            let v2 = problem.add_var(2.0, (0.0, 3.0));
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 5.0);

            let mut feas_checker = problem.create_feasibility_checker().unwrap();

            assert!(feas_checker.check_feasibility().is_err());
        }
    }

    #[test]
    fn set_bound() {
        {
            let mut problem = Problem::new(OptimizationDirection::Maximize);
            let v1 = problem.add_var(1.0, (0.0, 3.0));
            let v2 = problem.add_var(2.0, (0.0, 3.0));
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Le, 4.0);
            problem.add_constraint([(v1, 1.0), (v2, 1.0)], ComparisonOp::Ge, 1.0);

            let mut feas_checker = problem.create_feasibility_checker().unwrap();

            assert!(feas_checker.check_feasibility().is_ok());

            let r1 = feas_checker.set_bound(v2, &Bound::Lower, 2.0);
            let r2 = feas_checker.set_bound(v1, &Bound::Lower, 4.0);

            assert!(feas_checker.check_feasibility().is_err() || r1.is_err() || r2.is_err());

            let _ = feas_checker.set_bound(v1, &Bound::Lower, 2.0);

            assert!(feas_checker.check_feasibility().is_ok());
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
        mut problem: Problem,
        lit_vec: Vec<Lit>,
        nb_const: usize,
        verbose: bool,
        assert_on: bool,
    ) -> (usize, usize, usize, usize) {
        let init_feas_checker = problem.create_feasibility_checker();

        let init_solve = problem.solve();

        if init_feas_checker.is_err() {
            assert!(init_solve.is_err());
            return (0, 0, 0, 0);
        }

        let mut feas_checker = init_feas_checker.unwrap();

        if init_solve.is_err() {
            assert!(feas_checker.check_feasibility().is_err());
        }

        let mut solution = init_solve.unwrap();

        let mut nb_cert_gen_const = 0;
        let mut nb_val_cert_const = 0;

        let mut nb_cert_gen_feas = 0;
        let mut nb_val_cert_feas = 0;

        for (i, lit) in lit_vec.iter().enumerate() {
            problem.set_bound(lit.var, &lit.bound, lit.val);

            let res_set_bound = feas_checker.set_bound(lit.var, &lit.bound, lit.val);

            let res_check_feas = feas_checker.check_feasibility();

            let res_add_const = solution.add_constraint([(lit.var, 1.0)], lit.bound.into(), lit.val);

            if let Err(Error::InfeasibleWithCertificate(cert)) = &res_add_const {
                let is_certif_valid = problem.is_certificate_valid(&cert[..nb_const]);
                println!("Is certificate valid add_const: {}", is_certif_valid);
                nb_cert_gen_const += 1;
                nb_val_cert_const += is_certif_valid as usize;
            }

            if let Err(Error::InfeasibleWithCertificate(cert)) = &res_set_bound {
                let is_certif_valid = problem.is_certificate_valid(cert);
                println!("Is certificate valid set_bound: {}", is_certif_valid);
                nb_cert_gen_feas += 1;
                nb_val_cert_feas += is_certif_valid as usize;
            }

            if let Err(Error::InfeasibleWithCertificate(cert)) = &res_check_feas {
                let is_certif_valid = problem.is_certificate_valid(cert);
                println!("Is certificate valid cheak_feas: {}", is_certif_valid);
                nb_cert_gen_feas += 1;
                nb_val_cert_feas += is_certif_valid as usize;
            }

            if assert_on {
                assert_eq!(
                    res_add_const.is_err(),
                    res_check_feas.is_err() || res_set_bound.is_err()
                );
            }

            if res_add_const.is_err() || res_check_feas.is_err() || res_set_bound.is_err() {
                if verbose {
                    println!(
                        "{i}\nadd_const: {:?}\nset_bound: {:?}\ncheack_feas: {:?}",
                        res_add_const, res_set_bound, res_check_feas
                    );
                }
                break;
            }

            solution = res_add_const.unwrap();
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
                test_incremental_constraints(problem, lit_vec, nb_const, true, assert_on);
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
    fn rand_10_problems() {
        rand_problems(0, 1000, 50, 100, -10, 10, 0.05, false);
    }

    #[test]
    fn rand_i32_min_max_problems() {
        rand_problems(0, 1000, 50, 100, i32::MIN, i32::MAX, 0.05, false);
    }

    #[test]
    fn certificate() {
        {
            let mut problem = Problem::new(OptimizationDirection::Maximize);
            let x1 = problem.add_var(0.0, (0.0, f64::INFINITY));
            let x2 = problem.add_var(0.0, (0.0, f64::INFINITY));
            problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Le, 5.0);
            problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 10.0);

            let sol = problem.solve();

            assert!(sol.is_err());

            println!("{:?}", sol);

            if let Error::InfeasibleWithCertificate(cert) = sol.unwrap_err() {
                assert!(problem.is_certificate_valid(&cert));
            }
        }

        {
            let mut problem = Problem::new(OptimizationDirection::Maximize);
            let x1 = problem.add_var(0.0, (0.0, 3.0));
            let x2 = problem.add_var(0.0, (0.0, 4.0));
            problem.add_constraint([(x1, 1.0), (x2, 2.0)], ComparisonOp::Le, 15.0);
            problem.add_constraint([(x1, 1.0), (x2, 1.0)], ComparisonOp::Ge, 9.0);

            let sol = problem.solve();

            assert!(sol.is_err());

            println!("{:?}", sol);

            if let Error::InfeasibleWithCertificate(cert) = sol.unwrap_err() {
                assert!(problem.is_certificate_valid(&cert));
            }
        }

        {
            let mut problem = Problem::new(OptimizationDirection::Maximize);
            let x1 = problem.add_var(0.0, (0.0, 2.0));
            let x2 = problem.add_var(0.0, (0.0, 2.0));
            problem.add_constraint([(x1, 2.0), (x2, 1.0)], ComparisonOp::Ge, 5.0);
            problem.add_constraint([(x1, -1.0), (x2, 1.0)], ComparisonOp::Ge, 2.0);

            let sol = problem.solve();

            assert!(sol.is_err());

            println!("{:?}", sol);

            if let Error::InfeasibleWithCertificate(cert) = sol.unwrap_err() {
                assert!(problem.is_certificate_valid(&cert));
            }
        }
    }
}
