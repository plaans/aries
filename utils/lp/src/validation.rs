//! Verification of the solver's results before they are handed out.
//!
//! Whatever the solver concludes is checked here against its internal representation of
//! the problem before being reported: a result that does not check out would be a wrong
//! answer, and is reported as a numerical failure ([`Error::Unstable`]) instead.
//! (Recovering from a failed check — e.g. by re-solving from a fresh basis — is a
//! natural extension, not implemented today.)
//!
//! All checks work on the internal representation, in which every constraint is an
//! equality `a x + s = b`: each constraint row carries a slack variable whose bounds
//! encode the comparison operator (`s >= 0` for `<=`, `s <= 0` for `>=`, `s = 0` for
//! `=`). The comparison directions therefore require no special treatment: they are
//! variable bounds like any other, and a single kind of certificate is verified, for
//! both infeasibility and optimality, over all variables. In particular, the classical
//! sign requirements on the multipliers are not checked but implied: a wrongly-signed
//! multiplier leaves its slack free to compensate, and no contradiction can be derived.
//!
//! Tolerances are made relative to the magnitudes involved, so that rounding on large
//! quantities does not reject honest results.

use crate::solver::Solver;

/// Base tolerance of the checks.
const TOL: f64 = 1e-8;

/// Verifies the solver's current assignment of the variables: all variables within
/// their bounds, all constraint equalities satisfied, and the objective value consistent
/// with the assignment.
pub(crate) fn solution_is_valid(solver: &Solver) -> bool {
    let num_vars = solver.num_total_vars();

    // Variable bounds.
    for var in 0..num_vars {
        let val = *solver.get_value(var);
        if val.is_nan() {
            return false;
        }
        let tol = TOL * (1.0 + val.abs());
        if val < solver.orig_var_mins[var] - tol || val > solver.orig_var_maxs[var] + tol {
            return false;
        }
    }

    // Constraint equalities: A x = b. The residual tolerance grows with the magnitude of
    // the row's terms, where the rounding error accumulates.
    for (row, &rhs) in solver.orig_constraints.outer_iterator().zip(solver.orig_rhs.iter()) {
        let mut activity = 0.0;
        let mut magnitude = 0.0;
        for (var, &coef) in row.iter() {
            let term = coef * *solver.get_value(var);
            activity += term;
            magnitude += term.abs();
        }
        let tol = TOL * (1.0 + rhs.abs() + magnitude);
        if (activity - rhs).abs() > tol {
            return false;
        }
    }

    // Objective value: it is maintained incrementally by the solver, so it is checked
    // against a recomputation from the assignment.
    let mut objective = 0.0;
    for var in 0..num_vars {
        objective += solver.orig_obj_coeffs[var] * *solver.get_value(var);
    }
    let tol = TOL * (1.0 + objective.abs().max(solver.cur_obj_val.abs()));
    (objective - solver.cur_obj_val).abs() <= tol
}

/// Verifies an infeasibility certificate: the multipliers `y` (one per constraint) must
/// make the weighted equality `y^T (A x) = y^T b` contradict the variables' bounds — no
/// assignment within the bounds satisfies it.
///
/// Because the constraint rows are equalities, no comparison direction is involved: the
/// proof is that the right-hand side lies outside the range that the bounds impose on
/// the weighted combination.
pub(crate) fn infeasibility_certificate_is_valid(solver: &Solver, multipliers: &[f64]) -> bool {
    debug_assert_eq!(multipliers.len(), solver.num_constraints());

    // The weighted combination: `sum_r y_r * (a_r x)` and its right-hand side `y^T b`.
    let mut expr = vec![0.0; solver.num_total_vars()];
    let mut bound = 0.0;
    for (row_index, row) in solver.orig_constraints.outer_iterator().enumerate() {
        let y = multipliers[row_index];
        if y == 0.0 {
            continue;
        }
        bound += y * solver.orig_rhs[row_index];
        for (var, &coef) in row.iter() {
            expr[var] += y * coef;
        }
    }

    // Contradiction with the variables' bounds.
    max_over_bounds(solver, &expr) < bound || min_over_bounds(solver, &expr) > bound
}

/// Verifies an optimality certificate: the dual multipliers `y` of an optimal basis
/// (see [`Solver::dual_multipliers`]) together with the claimed minimal `objective`
/// value (in the solver's internal, minimized orientation).
///
/// Writing `d = c - A^T y` for the reduced costs they induce, every assignment within
/// the bounds satisfies `c^T x = y^T b + d^T x`, so the multipliers prove the bound
/// `c^T x >= y^T b + sum_j (minimal contribution of d_j over the bounds of x_j)`.
/// The certificate is valid if that bound proves the claimed objective: that nothing
/// better than it exists.
pub(crate) fn optimality_certificate_is_valid(solver: &Solver, multipliers: &[f64], objective: f64) -> bool {
    debug_assert_eq!(multipliers.len(), solver.num_constraints());

    // Reduced costs `d = c - A^T y` over all variables, and the right-hand side of the
    // weighted equality.
    let mut reduced = solver.orig_obj_coeffs.clone();
    let mut bound = 0.0;
    for (row_index, row) in solver.orig_constraints.outer_iterator().enumerate() {
        let y = multipliers[row_index];
        if y == 0.0 {
            continue;
        }
        bound += y * solver.orig_rhs[row_index];
        for (var, &coef) in row.iter() {
            reduced[var] -= y * coef;
        }
    }

    for var in 0..solver.num_total_vars() {
        let d = reduced[var];
        // The bound the reduced cost points to: the lowest value of d_j * x_j over the
        // bounds of x_j.
        let at_bound = if d >= 0.0 {
            solver.orig_var_mins[var]
        } else {
            solver.orig_var_maxs[var]
        };
        if at_bound.is_infinite() {
            // The reduced cost points towards an infinite bound: the bound it proves is
            // unbounded below unless the reduced cost is (numerically) zero. For a slack
            // variable this is where the sign requirements on the multipliers show up:
            // its reduced cost is minus its multiplier, and pointing outside the slack's
            // bounds means the multiplier has the wrong sign.
            if d.abs() > TOL {
                return false;
            }
        } else {
            bound += d * at_bound;
        }
    }

    bound >= objective - TOL * (1.0 + objective.abs() + bound.abs())
}

/// The maximum of `expr^T x` over the variables' bounds.
///
/// Coefficients of magnitude at most [`TOL`] are treated as null: they are numerical
/// noise whose contribution would be dwarfed by their uncertainty.
fn max_over_bounds(solver: &Solver, expr: &[f64]) -> f64 {
    expr.iter()
        .enumerate()
        .map(|(var, &coef)| {
            if coef.abs() <= TOL {
                0.0
            } else if coef > 0.0 {
                coef * solver.orig_var_maxs[var]
            } else {
                coef * solver.orig_var_mins[var]
            }
        })
        .sum()
}

/// The minimum of `expr^T x` over the variables' bounds (see [`max_over_bounds`]).
fn min_over_bounds(solver: &Solver, expr: &[f64]) -> f64 {
    expr.iter()
        .enumerate()
        .map(|(var, &coef)| {
            if coef.abs() <= TOL {
                0.0
            } else if coef > 0.0 {
                coef * solver.orig_var_mins[var]
            } else {
                coef * solver.orig_var_maxs[var]
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::to_sparse;
    use crate::prelude::*;

    /// The LP: minimize -x0 + 2*x1 subject to x0 + x1 >= 4 and x0 <= 5, with
    /// x0 in [0, inf) and x1 in [0, 3].
    ///
    /// Its optimum is -5 at (5, 0) (internal, minimized orientation). At creation the
    /// solver is neither primal nor dual feasible.
    fn adversarial_solver() -> Solver {
        Solver::try_new(
            &[-1.0, 2.0],
            &[0.0, 0.0],
            &[f64::INFINITY, 3.0],
            &[
                (to_sparse(&[1.0, 1.0]), ComparisonOp::Ge, 4.0),
                (to_sparse(&[1.0, 0.0]), ComparisonOp::Le, 5.0),
            ],
        )
        .unwrap()
    }

    #[test]
    fn solutions_are_validated() {
        let mut solver = adversarial_solver();

        // Before solving, the violated constraints leave basic variables out of their
        // bounds (the slack of the >= row), which the check detects.
        assert!(!solution_is_valid(&solver));

        // The solved assignment satisfies all bounds and equalities, and its objective
        // value is consistent.
        solver.initial_solve(Deadline::None).unwrap();
        assert!(solution_is_valid(&solver));
    }

    #[test]
    fn infeasibility_certificates_are_validated() {
        // x0 + x1 >= 3 with both vars in [0, 1]: infeasible.
        let mut solver = Solver::try_new(
            &[0.0, 0.0],
            &[0.0, 0.0],
            &[1.0, 1.0],
            &[(to_sparse(&[1.0, 1.0]), ComparisonOp::Ge, 3.0)],
        )
        .unwrap();

        // The solver only reports certificates that pass this check, so its own
        // certificate validates.
        let certificate = match solver.solve_feasibility(Deadline::None) {
            Err(Error::InfeasibleWithCertificate(certificate)) => certificate,
            other => panic!("expected an infeasibility certificate, got {other:?}"),
        };
        assert!(infeasibility_certificate_is_valid(&solver, &certificate));

        // Multipliers that prove nothing (all zero) do not validate.
        assert!(!infeasibility_certificate_is_valid(&solver, &[0.0]));
    }

    #[test]
    fn infeasibility_validation_accounts_for_slack_directions() {
        // The comparison directions of the constraints are encoded as bounds of the
        // slack variables: a certificate must contradict those, and wrongly-signed
        // multipliers leave the slacks free to compensate.
        //
        // A satisfiable problem: x <= 3 and x >= 1 with x in [0, 1.5] (e.g. x = 1).
        // Weighting both rows by 1 gives the equality 2x + s0 + s1 = 4, which the
        // bounds cannot contradict: the slacks absorb the difference. (An earlier
        // formulation of this check only kept the comparison direction of the last
        // weighted row and accepted these multipliers, certifying the infeasibility of
        // this satisfiable problem.)
        let solver = Solver::try_new(
            &[0.0],
            &[0.0],
            &[1.5],
            &[
                (to_sparse(&[1.0]), ComparisonOp::Le, 3.0),
                (to_sparse(&[1.0]), ComparisonOp::Ge, 1.0),
            ],
        )
        .unwrap();
        assert!(!infeasibility_certificate_is_valid(&solver, &[1.0, 1.0]));

        // An infeasible problem: x <= 0 and x >= 2 with x in [0, 1]. The mixed
        // multipliers [1, 1] still prove nothing (the slacks compensate); the solver's
        // own certificate does.
        let mut solver = Solver::try_new(
            &[0.0],
            &[0.0],
            &[1.0],
            &[
                (to_sparse(&[1.0]), ComparisonOp::Le, 0.0),
                (to_sparse(&[1.0]), ComparisonOp::Ge, 2.0),
            ],
        )
        .unwrap();
        assert!(!infeasibility_certificate_is_valid(&solver, &[1.0, 1.0]));
        match solver.solve_feasibility(Deadline::None) {
            Err(Error::InfeasibleWithCertificate(certificate)) => {
                assert!(infeasibility_certificate_is_valid(&solver, &certificate));
            }
            other => panic!("expected an infeasibility certificate, got {other:?}"),
        }
    }

    #[test]
    fn optimality_certificates_are_validated() {
        let mut solver = adversarial_solver();
        let duals = solver.initial_solve(Deadline::None).unwrap();

        // The dual multipliers prove the reported optimum (in the solver's internal,
        // minimized orientation).
        assert_eq!(solver.cur_obj_val, -5.0);
        assert!(optimality_certificate_is_valid(&solver, &duals, solver.cur_obj_val));

        // They do not support claiming a worse optimum: something better than it
        // could exist.
        assert!(!optimality_certificate_is_valid(
            &solver,
            &duals,
            solver.cur_obj_val + 1.0
        ));

        // Multipliers that prove nothing (all zero) do not validate: the objective is
        // unbounded below over the variables' bounds.
        let zeroed = vec![0.0; duals.len()];
        assert!(!optimality_certificate_is_valid(&solver, &zeroed, solver.cur_obj_val));
    }
}
