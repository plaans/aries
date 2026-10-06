use std::{collections::BinaryHeap, fmt};

use crate::{
    backtrack::{DecLvl, Trail},
    collections::ref_store::RefMap,
    core::{
        IntCst, Lit, LongCst, Var, cst_int_to_long,
        state::{Domains, DomainsSnapshot, Explanation},
    },
    reasoners::lp::{
        BoundConstraintId, BoundConstraintsStore, LpEvent, LpSum, Stats, VariableCounts,
        explanation_utils::{LbBoundEvent, SumElem},
    },
};

use aries_lp::{Bound, ComparisonOp, Error, FeasibilityChecker, OptimizationDirection, Problem, Variable};
#[allow(unused_imports)]
use itertools::Itertools;

/// How the current value of a bound was established, i.e. what justifies it in an explanation.
///
/// It is recorded in the bound history ([`IntBounds`]) whenever the bound is updated and restored on backtrack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundJustification {
    /// The bound holds unconditionally: initial bound of the variable,
    /// or bound of a trigger whose condition is entailed at the root of the main model.
    Unconditional,
    /// The bound was pushed by the synchronization with the main model, which entails it:
    /// `lit` is the literal of the main-model event that justified it.
    /// Only variables mapped to a var of the main model receive such bounds.
    CpModel(Lit),
    /// The bound was pushed by a bound-update trigger (see `Lp::add_bound_update_trigger`):
    /// the main model does not entail it, only `cause` justifies it.
    Trigger(BoundConstraintId),
}

impl BoundJustification {
    /// Pushes to `explanation` the literals that justify a bound with this justification.
    ///
    /// An unconditional bound contributes no literal: it holds at the root.
    pub fn push_to(self, explanation: &mut Explanation, bc_store: &BoundConstraintsStore) {
        match self {
            BoundJustification::Unconditional => {}
            BoundJustification::CpModel(lit) => explanation.push(lit),
            BoundJustification::Trigger(bc) => explanation.extend(&bc_store[bc].0),
        }
    }
}

/// Used to store the bounds of our variable and the justification of each of them (useful for explanations)
#[derive(Clone, PartialEq)]
pub(super) struct IntBounds {
    pub(super) lower: LongCst,
    pub(super) lower_justification: BoundJustification,
    pub(super) upper: LongCst,
    pub(super) upper_justification: BoundJustification,
}

impl fmt::Debug for IntBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{},{}]", self.lower, self.upper)
    }
}

/// Stores a constraint of our lp with integer coefficients, necessary to verify the certificate
///
/// No need to store a bound or an operator as all of our constraints are equalities between an s variable and linear sum of x variables
pub type IntegerConstraint = super::LpSum;

/// Interface with the aries-lp solver, also used to verify its certificates
#[derive(Clone)]
pub struct Solver {
    pub(super) problem: Problem,

    pub init_bounds: Vec<(LongCst, LongCst)>,
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
            init_bounds: Vec::new(),
            bounds: Vec::new(),
            constraints: Vec::new(),
            is_explanation_refined,
            map_lp_to_aries: RefMap::default(),
            opt_feas_checker: None,
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
                "Variable {} in the LP has important bounds, may compromise LP stability",
                var.idx()
            );
        }

        stats.num_variables += 1;

        debug_assert_eq!(var.idx(), self.bounds.len());

        self.init_bounds.push((lb, ub));
        self.bounds.push(IntBounds {
            lower: lb,
            upper: ub,
            // the initial bounds of a variable hold unconditionally
            lower_justification: BoundJustification::Unconditional,
            upper_justification: BoundJustification::Unconditional,
        });
        var
    }

    /// Returns always valid bounds for `var * factor`
    pub fn get_init_bounds(&self, factor: IntCst, var: Variable) -> (LongCst, LongCst) {
        let factor = cst_int_to_long(factor);
        let (lb, ub) = self.init_bounds[var.idx()];
        if factor >= 0 {
            (factor * lb, factor * ub)
        } else {
            (factor * ub, factor * lb)
        }
    }

    /// Set a new Upper/Lower bound for the given variable
    ///
    /// `justification` tells how the new bound was established (and it will be used to build the explanations involving it).
    /// `counts` is told whether the variable became fixed / unfixed.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediately detected as infeasible.
    pub fn set_bound(
        &mut self,
        var: Variable,
        bound: Bound,
        val: LongCst,
        justification: BoundJustification,
        counts: &mut VariableCounts,
    ) -> Result<(), Error> {
        if self.opt_feas_checker.is_none() {
            self.opt_feas_checker = Some(self.problem.create_feasibility_checker()?);
        }

        let feas_checker = self.opt_feas_checker.as_mut().unwrap();

        debug_assert!(var.idx() < self.bounds.len());

        let was_fixed = self.bounds[var.idx()].lower == self.bounds[var.idx()].upper;
        match bound {
            Bound::Lower => {
                self.bounds[var.idx()].lower = val;
                self.bounds[var.idx()].lower_justification = justification;
            }
            Bound::Upper => {
                self.bounds[var.idx()].upper = val;
                self.bounds[var.idx()].upper_justification = justification;
            }
        }
        let is_fixed = self.bounds[var.idx()].lower == self.bounds[var.idx()].upper;
        counts.update(var, was_fixed, is_fixed);

        self.problem.set_bound(var, &bound, val as f64);

        feas_checker.set_bound(var, &bound, val as f64)?;

        Ok(())
    }

    /// Same as [`Solver::set_bound_restrict`], but the change is *not* trailed:
    /// the bound will never be undone by a backtrack.
    ///
    /// Only valid for a `justification` that holds at the root, i.e. always / unconditionally.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediately detected as infeasible.
    pub fn set_bound_restrict_permanent(
        &mut self,
        var: Variable,
        bound: Bound,
        val: LongCst,
        counts: &mut VariableCounts,
    ) -> Result<bool, Error> {
        let restricts = match bound {
            Bound::Lower => val > self.bounds[var.idx()].lower,
            Bound::Upper => val < self.bounds[var.idx()].upper,
        };
        if restricts {
            self.set_bound(var, bound, val, BoundJustification::Unconditional, counts)?;
        }
        Ok(restricts)
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
        justification: BoundJustification,
        trail: &mut Trail<LpEvent>,
        counts: &mut VariableCounts,
    ) -> Result<bool, Error> {
        match bound {
            Bound::Lower => {
                let old_val = self.bounds[var.idx()].lower;
                let old_justification = self.bounds[var.idx()].lower_justification;
                if val > old_val {
                    trail.push(LpEvent {
                        var,
                        bound,
                        old_val,
                        old_justification,
                    });
                    self.set_bound(var, bound, val, justification, counts)?;

                    return Ok(true);
                }
            }
            Bound::Upper => {
                let old_val = self.bounds[var.idx()].upper;
                let old_justification = self.bounds[var.idx()].upper_justification;

                if val < old_val {
                    trail.push(LpEvent {
                        var,
                        bound,
                        old_val,
                        old_justification,
                    });
                    self.set_bound(var, bound, val, justification, counts)?;

                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Restore the feasibility of the lp solver
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
    pub fn add_constraint(&mut self, lin_sum: LpSum) {
        let float_lin_sum: Vec<(Variable, f64)> = lin_sum.iter().map(|(var, coef)| (var, coef as f64)).collect();

        if let Some(feas_checker) = self.opt_feas_checker.as_mut() {
            let res = feas_checker.add_constraint(&float_lin_sum, ComparisonOp::Eq, 0.0);
            // Adding a constraint that is not active yet should no generate an error
            assert!(res.is_ok())
        }

        self.problem.add_constraint(&float_lin_sum, ComparisonOp::Eq, 0.0);

        self.constraints.push(lin_sum);
    }

    /// Return the maximum value that the given linear sum can take to respect to its variable bounds
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
    pub fn check_certificate(
        &mut self,
        cert: &[f64],
        domains: &Domains,
        stats: &mut Stats,
        bc_store: &BoundConstraintsStore,
    ) -> Option<Explanation> {
        debug_assert_eq!(cert.len(), self.constraints.len());

        let mut lin_sum: Vec<i128> = vec![0; self.bounds.len()];

        let cert_i128 = Solver::convert_certificate_i128(cert);

        stats.num_overflow += 1;

        // We build the constraint that should be infeasible based on the certificate, it's only a linear sum as our constraints are equalities
        for (const_i, &coef_cert) in cert_i128.iter().enumerate() {
            if coef_cert == 0 {
                continue;
            }

            for (var_i, coef_var) in self.constraints[const_i].iter() {
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
                return Some(self.explain_geq(&lin_sum, domains, bc_store));
            } else {
                return Some(self.explain_geq_basic(&lin_sum, bc_store));
            }
        }

        if min_lin_sum > 0 {
            if self.is_explanation_refined {
                return Some(self.explain_leq(&lin_sum, domains, bc_store));
            } else {
                return Some(self.explain_leq_basic(&lin_sum, bc_store));
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
    /// How each variable is treated depends on the [`BoundJustification`] recorded in its bound history:
    /// - a bound pushed by the synchronization with the main model can only appear on a variable mapped with a [`Var`]
    ///   of the aries solver: the events of the main model's history justify it and are added to the culprits list.
    /// - any other bound (initial bound, or bound pushed by a bound-update trigger) contributes the literals
    ///   recorded with it and cancels its contribution to the ub.
    ///
    /// The last step consists of eliminating the culprits that are not necessary to explain the infeasibility
    /// and iterate in the past bound events to have an [`Explanation`] as minimal as possible.
    ///
    /// As an example, if the last bound event on x was `x <= 5` but `x <= 8` which was the previous bound event is sufficient for infeasibility,
    /// we add `x <= 8` to the [`Explanation`]
    ///
    /// This minimization takes more time to compute that basic explanations but in most of the cases, the clause learnt is stronger, allowing
    /// a better backtracking and pruning.
    ///
    /// Note that an auxiliary x variable never contributes a literal *of its own*:
    /// having no counterpart in the main model, no literal denotes its bounds.
    /// What it contributes are the literals of the [`BoundJustification`] recorded when a bound on it was set
    /// (which is what makes the bound valid in the first place).
    ///
    /// An auxiliary variable still holding its initial bounds contributes nothing, as they hold unconditionally.
    fn explain_leq(&self, lin_sum: &[i128], domains: &Domains, bc_store: &BoundConstraintsStore) -> Explanation {
        let mut explanation = Explanation::new();

        // We always explain a contradiction, i.e. lower bounds summing to *strictly* more than 0,
        // so the budget is raised by one to match the non-strict comparisons used below.
        // Same trick as in [`crate::reasoners::cp::linear::explain`].
        let mut ub = 1;

        let domain_snap = DomainsSnapshot::current(domains);

        let mut culprits = BinaryHeap::new();

        // We iterate over the lin_sum to eliminate variables with a null coef
        // Variables that are really present in the constraint (coef != 0) are then treated separately
        // depending on how their minimizing bound was established, as recorded in its justification.
        for (idx, &coef) in lin_sum.iter().enumerate() {
            if coef == 0 {
                continue;
            }

            // Bound of the LP variable in the direction that minimizes the sum
            // (lower bound for a positive coefficient, upper bound for a negative one)
            // together with the justification recorded when it was set.
            let (bound, justification) = if coef > 0 {
                let b = &self.bounds[idx];
                (b.lower, b.lower_justification)
            } else {
                let b = &self.bounds[idx];
                (b.upper, b.upper_justification)
            };

            match justification {
                // The bound was pushed by the synchronization with the main model, which entails it:
                // the events of the main model's history justify it and can be minimized.
                // The synchronization only concerns variables that are mapped to a var in aries solver.
                BoundJustification::CpModel(_) => {
                    let &var = self
                        .map_lp_to_aries
                        .get(idx)
                        .expect("a bound entailed by the main model can only be set on a variable mapped to it");
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
                        ub -= (elem_var_lb as i128).saturating_mul(sum_elem.factor);
                    }
                }
                // The bound either:
                //  - holds unconditionally (initial bound of the variable, or condition of a trigger entailed at the root), or
                //  - was pushed by a bound-update trigger: the literals recorded with it are the only justification of its value.
                _ => {
                    justification.push_to(&mut explanation, bc_store);
                    // the contribution of the bound to the sum is fully determined, it cancels from the UB
                    ub -= (bound as i128).saturating_mul(coef);
                }
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
    fn explain_geq(&self, lin_sum: &[i128], domains: &Domains, bc_store: &BoundConstraintsStore) -> Explanation {
        let opp_constraint = &lin_sum.iter().map(|&coef| -coef).collect_vec();

        self.explain_leq(opp_constraint, domains, bc_store)
    }

    /// Return a basic explanation containing all the [`Lit`] associated with each variable + bound present in the constraint `<lin_sum, variables> <= 0`
    ///
    /// lin_sum contains the coefficients for every lp [`Variable`] in the constraint (including null coefficients).
    /// The coefficient at index 0 is associated with the [`Variable`] of [`Variable::idx`] 0 and so on
    fn explain_leq_basic(&self, lin_sum: &[i128], bc_store: &BoundConstraintsStore) -> Explanation {
        let mut explanation = Explanation::new();

        for (i, &coeff) in lin_sum.iter().enumerate() {
            if coeff == 0 {
                continue;
            }
            let justification = if coeff < 0 {
                self.bounds[i].upper_justification
            } else {
                self.bounds[i].lower_justification
            };
            justification.push_to(&mut explanation, bc_store);
        }

        explanation
    }

    /// Return a basic explanation containing all the [`Lit`] associated with each variable + bound present in the constraint `<lin_sum, variables> >= 0`
    fn explain_geq_basic(&self, lin_sum: &[i128], bc_store: &BoundConstraintsStore) -> Explanation {
        let opp_constraint = &lin_sum.iter().map(|&coef| -coef).collect_vec();

        self.explain_leq_basic(opp_constraint, bc_store)
    }

    /// Return an explanation containing the upper and lower lit associated with a var, used to explain trivial errors (with no certificate)
    pub fn explain_infeasible_var(&self, var: Variable, bc_store: &BoundConstraintsStore) -> Explanation {
        let mut explanation = Explanation::new();

        let int_bound = &self.bounds[var.idx()];

        for justification in [int_bound.upper_justification, int_bound.lower_justification] {
            justification.push_to(&mut explanation, bc_store);
        }

        explanation
    }
}
