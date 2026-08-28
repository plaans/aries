use std::fmt;

use crate::{
    backtrack::Trail,
    core::{IntCst, Lit, LongCst, state::Explanation},
    reasoners::lp::{LpEvent, Stats, log::Logger},
};

use aries_env_param::EnvParam;
#[allow(unused_imports)]
use itertools::Itertools;
use minilp::{Bound, ComparisonOp, Error, FeasibilityChecker, OptimizationDirection, Problem, Variable};

const LOG_FOLDER: &str = "/home/mseraud/Documents/log/";

pub static LP_LOG_ENABLE: EnvParam<bool> = EnvParam::new("ARIES_LP_LOG_ENABLE", "false");
pub static LP_LOG_NAME: EnvParam<String> = EnvParam::new("ARIES_LP_LOG_NAME", "default.log");

/// Used to store the bounds of our variable and the associated Lit that is responsible of these bounds (useful for explanations)
#[derive(Clone, PartialEq)]
pub(super) struct IntBounds {
    lower: LongCst,
    lower_lit: Lit,
    upper: LongCst,
    upper_lit: Lit,
}

impl fmt::Debug for IntBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{},{}]", self.lower, self.upper)
    }
}

// No need to store a bound or an operator as all of our constraints are equalities between an s variable and linear sum of x variables
#[derive(Debug, Clone, PartialEq)]
pub struct IntegerConstraint {
    lin_sum: Vec<(Variable, IntCst)>,
}

#[derive(Clone)]
pub struct Solver {
    pub(super) problem: Problem,

    // Used to store an exact version of our original problem with integers
    pub(super) bounds: Vec<IntBounds>,
    pub(super) constraints: Vec<IntegerConstraint>,

    opt_feas_checker: Option<FeasibilityChecker>,

    pub(super) logger: Logger,
    is_first_invalid_cert: bool,
}

impl PartialEq for Solver {
    fn eq(&self, other: &Self) -> bool {
        self.bounds == other.bounds && self.constraints == other.constraints && self.problem == other.problem
    }
}

impl Solver {
    pub fn new() -> Self {
        Solver {
            problem: Problem::new(OptimizationDirection::Maximize),
            bounds: Vec::new(),
            constraints: Vec::new(),
            opt_feas_checker: None,
            logger: Logger::new(),
            is_first_invalid_cert: true,
        }
    }

    /// Reload the minilp solver based on the current problem state
    pub fn reload(&mut self) {
        self.opt_feas_checker = None;
    }

    /// Create a new solver variable both for Problem and the mirror of our problem
    pub fn create_variable(&mut self, lb: LongCst, ub: LongCst, stats: &mut Stats) -> Variable {
        let var = self.problem.add_var(0.0, (lb as f64, ub as f64));

        // If the minilp instance is already created, we need to update it
        if let Some(feas_checker) = self.opt_feas_checker.as_mut() {
            let res = feas_checker.add_variable(0.0, lb as f64, ub as f64);
            // Adding a variable should not generate an error as it would mean that we are trying to add a variable with inconsistent bounds
            assert!(res.is_ok());

            let idx_var = res.unwrap();

            assert_eq!(idx_var, var.idx());
        }

        const TRESHOLD_WARNING: i128 = 1_i128 << 40; // Experimentally computed

        if (lb as i128) > TRESHOLD_WARNING
            || (lb as i128) < -TRESHOLD_WARNING
            || (ub as i128) > TRESHOLD_WARNING
            || (ub as i128) < -TRESHOLD_WARNING
        {
            tracing::warn!(
                "Variable {} in the LP has important bounds, LP stability isn't expected",
                var.idx()
            );
        }

        stats.num_variables += 1;

        debug_assert_eq!(var.idx(), self.bounds.len());

        self.bounds.push(IntBounds {
            lower: lb,
            upper: ub,
            lower_lit: Lit::TRUE,
            upper_lit: Lit::TRUE,
        });
        var
    }

    /// Set a new Upper/Lower bound for the given variable
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediatly detected as infeasible.
    pub fn set_bound(&mut self, var: Variable, bound: Bound, val: LongCst, lit: Lit) -> Result<(), Error> {
        if self.opt_feas_checker.is_none() {
            self.opt_feas_checker = Some(self.problem.create_feasibility_checker()?);
        }

        let feas_checker = self.opt_feas_checker.as_mut().unwrap();

        debug_assert!(var.idx() < self.bounds.len());

        // TODO: conditonal logging with a feature
        self.logger.stack_event.push_event(var, bound, val as f64);

        match bound {
            Bound::Lower => {
                self.bounds[var.idx()].lower = val;
                self.bounds[var.idx()].lower_lit = lit;
            }
            Bound::Upper => {
                self.bounds[var.idx()].upper = val;
                self.bounds[var.idx()].upper_lit = lit;
            }
        }

        self.problem.set_bound(var, &bound, val as f64);

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
        val: LongCst,
        lit: Lit,
        trail: &mut Trail<LpEvent>,
    ) -> Result<(), Error> {
        match bound {
            Bound::Lower => {
                let old_val = self.bounds[var.idx()].lower;
                let old_lit = self.bounds[var.idx()].lower_lit;
                if val > old_val {
                    trail.push(LpEvent {
                        var,
                        bound,
                        old_val,
                        old_lit,
                    });
                    self.set_bound(var, bound, val, lit)?;
                }
            }
            Bound::Upper => {
                let old_val = self.bounds[var.idx()].upper;
                let old_lit = self.bounds[var.idx()].upper_lit;

                if val < old_val {
                    trail.push(LpEvent {
                        var,
                        bound,
                        old_val,
                        old_lit,
                    });
                    self.set_bound(var, bound, val, lit)?;
                }
            }
        }

        Ok(())
    }

    /// Restore the feasibilty of the lp solver
    ///
    /// # Errors
    ///
    /// Will return an error if it can't be restored
    pub fn check_feasibility(&mut self) -> Result<(), Error> {
        if self.opt_feas_checker.is_none() {
            self.opt_feas_checker = Some(self.problem.create_feasibility_checker()?);
        }

        let feas_checker = self.opt_feas_checker.as_mut().unwrap();

        feas_checker.check_feasibility()?;

        Ok(())
    }

    /// Add a new constraint in both float and integer problems
    pub fn add_constraint(&mut self, lin_sum: Vec<(Variable, IntCst)>) {
        let float_lin_sum: Vec<(Variable, f64)> = lin_sum.iter().map(|&(var, coef)| (var, coef as f64)).collect();

        if let Some(feas_checker) = self.opt_feas_checker.as_mut() {
            let res = feas_checker.add_constraint(&float_lin_sum, ComparisonOp::Eq, 0.0);
            // Adding a constraint that is not active yet should no generate an error
            assert!(res.is_ok())
        }

        self.problem.add_constraint(&float_lin_sum, ComparisonOp::Eq, 0.0);

        self.constraints.push(IntegerConstraint { lin_sum });
    }

    /// Return the maximum value that the given linear sum can take respect to its bounds
    fn max_lin_sum(&self, lin_sum: &[i128]) -> Option<i128> {
        lin_sum.iter().enumerate().try_fold(0i128, |acc, (i, &coeff)| {
            if coeff == 0 {
                Some(acc)
            } else {
                let bound = if coeff < 0 {
                    self.bounds[i].lower as i128
                } else {
                    self.bounds[i].upper as i128
                };

                let prod = bound.checked_mul(coeff)?;

                acc.checked_add(prod)
            }
        })
    }

    /// Return the minimum value that the given linear sum can take respect to its bounds
    fn min_lin_sum(&self, lin_sum: &[i128]) -> Option<i128> {
        lin_sum.iter().enumerate().try_fold(0i128, |acc, (i, &coeff)| {
            if coeff == 0 {
                Some(acc)
            } else {
                let bound = if coeff > 0 {
                    self.bounds[i].lower as i128
                } else {
                    self.bounds[i].upper as i128
                };

                let prod = bound.checked_mul(coeff)?;

                acc.checked_add(prod)
            }
        })
    }

    /// Convert a certificate with f64 coefficients in an a equivalent i128 certificate
    fn convert_certificate_i128(cert: &[f64]) -> Vec<i128> {
        let coef = 2.0_f64.powi(52); // 52

        cert.iter().map(|x| (x * coef).trunc() as i128).collect()
    }

    /// Verify the certificate of unsatisfiability
    ///
    /// Returns None if the certificate isn't valid, otherwise returns the Explanation of unsatisfiability
    pub fn check_certificate(&mut self, cert: &[f64], stats: &mut Stats) -> Option<Explanation> {
        debug_assert_eq!(cert.len(), self.constraints.len());

        let mut lin_sum: Vec<i128> = vec![0; self.bounds.len()];

        let cert_i128 = Solver::convert_certificate_i128(cert);

        stats.num_overflow += 1;

        // We build the constraint that should be infeasible based on the certificate, it's only a linear sum as our constraints are equalities
        for (const_i, &coef_cert) in cert_i128.iter().enumerate() {
            if coef_cert == 0 {
                continue;
            }

            for &(var_i, coef_var) in self.constraints[const_i].lin_sum.iter() {
                let prod = coef_cert.checked_mul(coef_var as i128)?;
                lin_sum[var_i.idx()] = lin_sum[var_i.idx()].checked_add(prod)?;
            }
        }

        let min_lin_sum = self.min_lin_sum(&lin_sum)?;
        let max_lin_sum = self.max_lin_sum(&lin_sum)?;

        stats.num_overflow -= 1;

        // To detect the infeasibility, we check if 0 is in the range [min, max] as our linear sum should be equal to 0
        if max_lin_sum < 0 {
            let mut explanation = Explanation::new();

            explanation.lits = lin_sum
                .iter()
                .enumerate()
                .filter(|&(_, &coeff)| coeff != 0)
                .map(|(i, &coeff)| {
                    if coeff > 0 {
                        self.bounds[i].upper_lit
                    } else {
                        self.bounds[i].lower_lit
                    }
                })
                .collect();

            return Some(explanation);
        }

        if min_lin_sum > 0 {
            let mut explanation = Explanation::new();

            explanation.lits = lin_sum
                .iter()
                .enumerate()
                .filter(|&(_, &coeff)| coeff != 0)
                .map(|(i, &coeff)| {
                    if coeff < 0 {
                        self.bounds[i].upper_lit
                    } else {
                        self.bounds[i].lower_lit
                    }
                })
                .collect();

            return Some(explanation);
        }

        // println!("Int cert max: {max_lin_sum}, min: {min_lin_sum}");
        // let min_coeff = lin_sum.iter().filter(|v| **v != 0).map(|v| v.abs()).min().unwrap();
        // println!(
        //     "Resulting constraint: {:?}",
        //     lin_sum
        //         .iter()
        //         .enumerate()
        //         .filter(|(_, v)| **v != 0)
        //         .map(|(i, v)| (i, *v as f64 / min_coeff as f64, &self.bounds[i]))
        //         .collect_vec()
        // );

        if LP_LOG_ENABLE.get() && self.is_first_invalid_cert && !self.problem.is_certificate_valid(cert) {
            self.is_first_invalid_cert = false;
            self.logger
                .save_to(format!("{LOG_FOLDER}{}", LP_LOG_NAME.get_ref()).as_str())
                .expect("Error while logging");
        }

        None
    }

    /// Return an explanation containing the upper and lower lit associated with a var, used to explain trivial errors (with no certificate)
    pub fn explain_infeasible_var(&self, var: Variable) -> Explanation {
        let mut explanation = Explanation::new();

        let int_bound = &self.bounds[var.idx()];

        explanation.push(int_bound.upper_lit);
        explanation.push(int_bound.lower_lit);

        explanation
    }
}
