use crate::{
    backtrack::Trail,
    core::IntCst,
    reasoners::lp::{ActivationLit, LpEvent},
};

use minilp::{Bound, ComparisonOp, Error, FeasabilityChecker, OptimizationDirection, Problem, Variable};

#[derive(Debug)]
struct IntBounds {
    lower: IntCst,
    upper: IntCst,
}

// No need to store a bound or an op as all of our constraints are equalities between an s variable and linear sum of x variables
#[derive(Debug)]
struct IntegerConstraint {
    lin_sum: Vec<(Variable, IntCst)>,
}

pub struct Solver {
    pub problem: Problem,

    // Used to store an exact version of our original problem with integers
    bounds: Vec<IntBounds>,
    constraints: Vec<IntegerConstraint>,

    opt_feas_checker: Option<FeasabilityChecker>,
}

impl Solver {
    pub fn new() -> Self {
        Solver {
            problem: Problem::new(OptimizationDirection::Maximize),
            bounds: Vec::new(),
            constraints: Vec::new(),
            opt_feas_checker: None,
        }
    }

    // Create a new solver variable both for Problem and the mirror of our problem
    pub fn create_variable(&mut self, lb: IntCst, ub: IntCst) -> Variable {
        let var = self.problem.add_var(0.0, (lb as f64, ub as f64));

        debug_assert_eq!(var.idx(), self.bounds.len());

        self.bounds.push(IntBounds { lower: lb, upper: ub });
        var
    }

    // Modify the bound of an existing variable
    pub fn set_bound(&mut self, var: Variable, bound: Bound, val: IntCst) -> Result<(), Error> {
        if self.opt_feas_checker.is_none() {
            self.opt_feas_checker = Some(self.problem.create_feasability_checker()?);
        }

        let feas_checker = self.opt_feas_checker.as_mut().unwrap();

        debug_assert!(var.idx() < self.bounds.len());

        match bound {
            Bound::Lower => self.bounds[var.idx()].lower = val,
            Bound::Upper => self.bounds[var.idx()].upper = val,
        }

        self.problem.set_bound(var, &bound, val as f64); // Used for test only

        feas_checker.set_bound(var, &bound, val as f64)?;

        Ok(())
    }

    /// Set a new Upper/Lower bound for the given variable if it is more restrictive than the old bound
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediatly detected as infeasible.
    pub fn set_bound_restrict(
        &mut self,
        var: Variable,
        bound: Bound,
        val: IntCst,
        trail: &mut Trail<LpEvent>,
    ) -> Result<(), Error> {
        let mut is_bound_set = false;

        let old_val;

        match bound {
            Bound::Lower => {
                old_val = self.bounds[var.idx()].lower;
                if val > old_val {
                    self.set_bound(var, bound, val)?;
                    is_bound_set = true;
                }
            }
            Bound::Upper => {
                old_val = self.bounds[var.idx()].upper;

                if val < old_val {
                    self.set_bound(var, bound, val)?;
                    is_bound_set = true;
                }
            }
        }

        if is_bound_set {
            trail.push(LpEvent::BoundSet(ActivationLit { var, bound, val }, old_val));
        }

        Ok(())
    }

    pub fn check_feasability(&mut self) -> Result<(), Error> {
        if self.opt_feas_checker.is_none() {
            self.opt_feas_checker = Some(self.problem.create_feasability_checker()?);
        }

        let feas_checker = self.opt_feas_checker.as_mut().unwrap();

        feas_checker.check_feasability()?;

        Ok(())
    }

    // Create a new solver constraint both for Problem and the mirror of our problem, we assume we only have equality constraint
    pub fn add_constraint(&mut self, lin_sum: Vec<(Variable, IntCst)>) {
        let float_lin_sum: Vec<(Variable, f64)> = lin_sum.iter().map(|&(var, coef)| (var, coef as f64)).collect();

        self.problem.add_constraint(float_lin_sum, ComparisonOp::Eq, 0.0);

        self.constraints.push(IntegerConstraint { lin_sum });
    }

    // Return the maximum value that the given linear sum can take respect to its bounds
    fn max_lin_sum(&self, lin_sum: &[i128]) -> i128 {
        lin_sum
            .iter()
            .enumerate()
            .map(|(i, &coeff)| {
                if coeff == 0 {
                    0
                } else if coeff < 0 {
                    self.bounds[i].lower as i128 * coeff
                } else {
                    self.bounds[i].upper as i128 * coeff
                }
            })
            .sum()
    }

    // Return the minimum value that the given linear sum can take respect to its bounds
    fn min_lin_sum(&self, lin_sum: &[i128]) -> i128 {
        lin_sum
            .iter()
            .enumerate()
            .map(|(i, &coeff)| {
                if coeff == 0 {
                    0
                } else if coeff > 0 {
                    self.bounds[i].lower as i128 * coeff
                } else {
                    self.bounds[i].upper as i128 * coeff
                }
            })
            .sum()
    }

    // Convert a certificate with f64 coefficients in an a equivalent i128 certificate
    fn convert_certificate_i128(cert: &[f64]) -> Vec<i128> {
        let coef = 2.0_f64.powi(52);

        cert.iter().map(|x| (x * coef) as i128).collect()
    }

    /// Verify the certificate of unsatisfiability
    pub fn is_certificate_valid(&self, cert: &[f64]) -> bool {
        debug_assert_eq!(cert.len(), self.constraints.len());

        let mut lin_sum: Vec<i128> = vec![0; self.bounds.len()];

        let cert_i128 = Solver::convert_certificate_i128(cert);

        for (const_i, &coef_cert) in cert_i128.iter().enumerate() {
            if coef_cert == 0 {
                continue;
            }

            for &(var_i, coef_var) in self.constraints[const_i].lin_sum.iter() {
                lin_sum[var_i.idx()] += coef_cert * coef_var as i128;
            }
        }

        let min_lin_sum = self.min_lin_sum(&lin_sum);
        let max_lin_sum = self.max_lin_sum(&lin_sum);

        // println!("Int cert max: {max_lin_sum}, min: {min_lin_sum}");

        max_lin_sum < 0 || min_lin_sum > 0
    }
}
