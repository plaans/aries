use std::{collections::BinaryHeap, fmt};

use crate::{
    backtrack::{DecLvl, Trail},
    collections::ref_store::RefMap,
    core::{
        IntCst, Lit, LongCst, Var,
        state::{Domains, DomainsSnapshot, Explanation},
    },
    reasoners::lp::{
        LpEvent, Stats,
        explanation_lang::{LbBoundEvent, SumElem},
    },
};

#[cfg(feature = "lp_log")]
use crate::reasoners::lp::log::{LOG_FOLDER, LP_LOG_ENABLE, LP_LOG_NAME, Logger};

use aries_lp::{Bound, ComparisonOp, Error, FeasibilityChecker, OptimizationDirection, Problem, Variable};
#[allow(unused_imports)]
use itertools::Itertools;

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

/// Stores a constraint of our lp with integer coeficients, necesary to verify the certificate
///
/// No need to store a bound or an operator as all of our constraints are equalities between an s variable and linear sum of x variables
#[derive(Debug, Clone, PartialEq)]
pub struct IntegerConstraint {
    lin_sum: Vec<(Variable, IntCst)>,
}

/// Interface with the aries-lp solver, also used to verify its certificates
#[derive(Clone)]
pub struct Solver {
    pub(super) problem: Problem,

    /// Used to store an exact version of our original problem with integers
    pub(super) bounds: Vec<IntBounds>,
    pub(super) constraints: Vec<IntegerConstraint>,

    /// If true, refined explanation are used with minimization based on [`crate::reasoners::cp::linear`]
    pub(super) is_explanation_refined: bool,

    /// Maps aries-lp [`Variable`] with their corresponding [`Var`] in aries solver (therefore slack variables do not appear)
    ///
    /// Variable can't be used directly as the key, therefore we use their associated index: [`Variable::idx()`]
    pub(super) map_lp_to_aries: RefMap<usize, Var>,

    opt_feas_checker: Option<FeasibilityChecker>,

    #[cfg(feature = "lp_log")]
    pub(super) logger: Logger,
    #[cfg(feature = "lp_log")]
    is_first_invalid_cert: bool,
}

impl PartialEq for Solver {
    fn eq(&self, other: &Self) -> bool {
        self.bounds == other.bounds && self.constraints == other.constraints && self.problem == other.problem
    }
}

impl Solver {
    pub fn new(is_explanation_refined: bool) -> Self {
        Solver {
            problem: Problem::new(OptimizationDirection::Maximize),
            bounds: Vec::new(),
            constraints: Vec::new(),
            is_explanation_refined,
            map_lp_to_aries: RefMap::default(),
            opt_feas_checker: None,
            #[cfg(feature = "lp_log")]
            logger: Logger::new(),
            #[cfg(feature = "lp_log")]
            is_first_invalid_cert: true,
        }
    }

    /// Create a new solver variable both for Problem and the mirror of our problem
    pub fn create_variable(&mut self, lb: LongCst, ub: LongCst, stats: &mut Stats) -> Variable {
        let var = self.problem.add_var(0.0, (lb as f64, ub as f64));

        // If the aries-lp instance is already created, we need to update it
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

        #[cfg(feature = "lp_log")]
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
    /// Returns true/false whether if the bound was effectively modified or not
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
    ) -> Result<bool, Error> {
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

                    return Ok(true);
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

                    return Ok(true);
                }
            }
        }

        Ok(false)
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
    pub fn check_certificate(&mut self, cert: &[f64], domains: &Domains, stats: &mut Stats) -> Option<Explanation> {
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
            if self.is_explanation_refined {
                return Some(self.explain_geq(&lin_sum, domains));
            } else {
                return Some(self.explain_geq_basic(&lin_sum));
            }
        }

        if min_lin_sum > 0 {
            if self.is_explanation_refined {
                return Some(self.explain_leq(&lin_sum, domains));
            } else {
                return Some(self.explain_leq_basic(&lin_sum));
            }
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

        #[cfg(feature = "lp_log")]
        {
            // Log the execution when the first invalid certificate is detected (both float and integer invalidity)
            if LP_LOG_ENABLE.get() && self.is_first_invalid_cert {
                self.is_first_invalid_cert = false;
                self.logger
                    .save_to(format!("{LOG_FOLDER}{}", LP_LOG_NAME.get_ref()).as_str())
                    .expect("Error while logging");
            }
        }

        None
    }

    /// Return a minimal [`Explanation`] inspired by [`crate::reasoners::cp::linear`] for the infeasible constraint `<lin_sum, variables> <= 0`
    ///
    /// lin_sum contains the coefficients for every lp [`Variable`] in the constraint (including null coefficients).
    /// The coefficient at index 0 is associated with the [`Variable`] of [`Variable::idx`] 0 and so on
    ///
    /// As some of the aries-lp [`Variable`] are slack variables, they do not appear in the aries solver, therefore
    /// we start be canceling their contribution to the ub and we update the [`Explanation`] to take into account the activation
    /// [`Lit`] that are responsible of the bounds of the slack [`Variable`]
    ///
    /// For [`Variable`] that are mapped with a [`Var`] in aries solver, we check if their bound is entailed at the root, if yes
    /// we cancel their contribution to the ub. For the others, we add their last bound event to the culprits list.
    ///
    /// The last step consists of eliminating the culprits that are not necessary to explain the infeasibility
    /// and iterate in the past bound events to have an [`Explanation`] as minimal as possible.
    ///
    /// As an example, if the last bound event on x was `x <= 5` but `x <= 8` which was the previous bound event is sufficient for infeasibility,
    /// we add `x <= 8` to the [`Explanation`]
    ///
    /// This minimization takes more time to compute that basic explanations but in most of the cases, the clause learnt is stronger, allowing
    /// a better backtracking and pruning.
    fn explain_leq(&self, lin_sum: &[i128], domains: &Domains) -> Explanation {
        let mut explanation = Explanation::new();

        let mut ub = 0;

        let domain_snap = DomainsSnapshot::current(domains);

        let mut culprits = BinaryHeap::new();

        // We iterate over the lin_sum to eliminate variables with a null coef
        // Variables that are really present in the constraint (coef != 0) are then treated separatly
        // depending if they are mapped to var in aries solver or if they are just slack variables.
        for (idx, &coef) in lin_sum.iter().enumerate() {
            if coef == 0 {
                continue;
            }

            // Check if it's a slack variable or not
            if let Some(&var) = self.map_lp_to_aries.get(idx) {
                let sum_elem = SumElem::new(coef, var);

                if let Some(event) = LbBoundEvent::new(sum_elem, &domain_snap) {
                    // there is a lower bound event on this element, add it to the set of culprits for later processing
                    culprits.push(event)
                } else {
                    // no event associated to the element, which means its value is entailed at the ROOT
                    // Hence it does need to be present in the explanation, but should cancel its contribution to the UB
                    let elem_var_lb = domains.lb(sum_elem.var);
                    debug_assert_eq!(
                        domains.entailing_level(Lit::geq(sum_elem.var, elem_var_lb as IntCst)),
                        DecLvl::ROOT
                    );
                    let elem_lb = (elem_var_lb as i128).saturating_mul(sum_elem.factor);
                    ub -= elem_lb;
                }
            } else {
                // We add the activation Lit associted with the bound of our slack variable and we cancel its contribution to the ub
                let lit = if coef > 0 {
                    ub -= coef * self.bounds[idx].lower as i128;
                    self.bounds[idx].lower_lit
                } else {
                    ub -= coef * self.bounds[idx].upper as i128;
                    self.bounds[idx].upper_lit
                };

                explanation.push(lit);
            }
        }

        let sum_lb = |culps: &BinaryHeap<LbBoundEvent>| -> i128 { culps.iter().map(|e| e.lb()).sum() };
        #[allow(unused)]
        let print = |culps: &BinaryHeap<LbBoundEvent>| {
            println!("QUEUE:");
            for e in culps.iter() {
                println!(
                    " {:?} ({:?}) {:?} {:?}    {:?}",
                    e.literal(),
                    e.elem,
                    e.lb(),
                    e.previous_lb(),
                    e.event
                )
            }
        };
        // print(&culprits);

        let mut culprits_lb = sum_lb(&culprits);
        // println!("BEFORE LOOP: {culprits_lb}   <= {ub}");
        while let Some(elem_event) = culprits.pop() {
            // let e = &elem_event;
            // println!(
            //     " {:?} ({:?}) {:?} {:?}    {:?}",
            //     e.literal(),
            //     &e.elem,
            //     e.lb(),
            //     e.previous_lb(),
            //     e.event
            // );
            let event_idx = elem_event.event;
            let lb = elem_event.lb();
            let prev_lb = elem_event.previous_lb();
            culprits_lb -= lb; // update the new lb after removing the LbBoundEvent
            debug_assert_eq!(culprits_lb, sum_lb(&culprits));

            debug_assert!(ub <= culprits_lb + lb);
            if ub <= culprits_lb + prev_lb {
                // this event is not necessary and considering the previous one would be sufficient for the explanation
                if let Some(previous) = elem_event.into_previous() {
                    // add the previous lower bound for later processing
                    culprits.push(previous);
                    culprits_lb += prev_lb;
                    // println!("  > to prev")
                } else {
                    // there was no previous event (ie the previous lower bound always holds)
                    debug_assert_eq!(
                        domains.entailing_level(domains.get_event(event_idx).previous_literal()),
                        DecLvl::ROOT
                    );
                    // no need to add to the explanation (tautology) but cancel its contribution
                    ub -= prev_lb;
                    // println!("  > folded")
                }
            } else {
                // this event is necessary, add it to the explanation
                explanation.push(elem_event.literal());
                ub -= lb;
                // println!("  > select")
            }
        }

        explanation
    }

    /// Return a minimal explanation inspired by [`crate::reasoners::cp::linear`] for the infeasible constraint `<lin_sum, variables> => 0`
    ///
    /// Check [`Solver::explain_leq`] for more details on how these [`Explanation`] are generated
    fn explain_geq(&self, lin_sum: &[i128], domains: &Domains) -> Explanation {
        let opp_constraint = &lin_sum.iter().map(|&coef| -coef).collect_vec();

        self.explain_leq(opp_constraint, domains)
    }

    /// Return a basic explanation containing all the [`Lit`] associated with each variable + bound present in the constraint `<lin_sum, variables> <= 0`
    ///
    /// lin_sum contains the coefficients for every lp [`Variable`] in the constraint (including null coefficients).
    /// The coefficient at index 0 is associated with the [`Variable`] of [`Variable::idx`] 0 and so on
    fn explain_leq_basic(&self, lin_sum: &[i128]) -> Explanation {
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

        explanation
    }

    /// Return a basic explanation containing all the [`Lit`] associated with each variable + bound present in the constraint `<lin_sum, variables> => 0`
    fn explain_geq_basic(&self, lin_sum: &[i128]) -> Explanation {
        let opp_constraint = &lin_sum.iter().map(|&coef| -coef).collect_vec();

        self.explain_leq_basic(opp_constraint)
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
