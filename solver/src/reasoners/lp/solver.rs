use std::{collections::BinaryHeap, fmt};

use aries_lp::prelude::*;

use crate::backtrack::{DecLvl, Trail};
use crate::collections::ref_store::RefMap;
use crate::core::{
    LongCst, cst_int_to_long,
    state::{DomainsSnapshot, Explanation},
};
use crate::prelude::*;
use crate::reasoners::lp::explanation_utils::{LbBoundEvent, SumElem};
use crate::reasoners::lp::{BoundConstraintId, BoundConstraintsStore, BoundRestriction, LpEvent, LpSum, Stats};

use itertools::Itertools;

/// How the current value of a bound was established, i.e. what justifies it in an explanation.
///
/// It is stored with the bound in [`IntBounds`] whenever the bound is updated, and restored with it on backtrack.
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
            BoundJustification::Trigger(bc) => explanation.extend(&bc_store.store[bc].0),
        }
    }
}
/// The current bounds of an LP variable, each with its justification (for explanations).
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

/// The bounds of the LP variables, by [`Variable::idx`].
#[derive(Clone, Default)]
pub(super) struct Bounds {
    /// The current bounds, with their justifications.
    pub(super) current: Vec<IntBounds>,
    /// The bounds that hold unconditionally (at the root): the initial ones,
    /// or tighter ones from bound constraints whose trigger holds at the root.
    pub(super) unconditional: Vec<(LongCst, LongCst)>,
    /// The initial bounds with which a variable was added.
    init: Vec<(LongCst, LongCst)>,
}
impl Bounds {
    fn push(&mut self, lb: LongCst, ub: LongCst) {
        self.unconditional.push((lb, ub));
        self.init.push((lb, ub));
        self.current.push(IntBounds {
            lower: lb,
            upper: ub,
            // the initial bounds of a variable hold unconditionally
            lower_justification: BoundJustification::Unconditional,
            upper_justification: BoundJustification::Unconditional,
        });
    }
}

/// A constraint of the LP with integer coefficients, to validate the certificates exactly.
///
/// All the constraints are equalities between a reification variable and a linear sum (`sum = 0`), so neither a bound nor an operator is stored.
pub type IntegerConstraint = super::LpSum;

/// The aries-lp solver, with an exact integer copy of its constraints and bounds, to validate and explain its certificates.
#[derive(Clone)]
pub struct Solver {
    pub(super) lp: Lp,

    pub(super) bounds: Bounds,
    pub(super) constraints: Vec<IntegerConstraint>,

    /// If true, explanations are refined (see [`Solver::explain_leq`]) using a minimization based on [`crate::reasoners::cp::linear`].
    pub(super) is_explanation_refined: bool,

    /// Maps the LP variables to the model variables they mirror (auxiliary and reification variables have none).
    ///
    /// Keyed by [`Variable::idx()`], as [`Variable`] can't be used directly as a key.
    pub(super) lp_to_model: RefMap<usize, Var>,
    /// Maps the model variables to the LP variables that mirror them (for those in posted constraints).
    pub(super) model_to_lp: RefMap<Var, Variable>,
}

impl PartialEq for Solver {
    fn eq(&self, other: &Self) -> bool {
        self.bounds.current == other.bounds.current && self.constraints == other.constraints && self.lp == other.lp
    }
}

impl Solver {
    pub fn new(is_explanation_refined: bool) -> Self {
        Solver {
            lp: Lp::new(OptimizationDirection::Maximize),
            bounds: Default::default(),
            constraints: Vec::new(),
            is_explanation_refined,
            lp_to_model: RefMap::default(),
            model_to_lp: RefMap::default(),
        }
    }

    /// Creates an LP variable, both in the LP and in its exact integer copy.
    pub fn create_variable(&mut self, lb: LongCst, ub: LongCst, stats: &mut Stats) -> Variable {
        let var = self.lp.add_var(0.0, (lb as f64, ub as f64));

        const THRESHOLD_WARNING: i128 = 1_i128 << 40; // Experimentally computed

        if (lb as i128) > THRESHOLD_WARNING
            || (lb as i128) < -THRESHOLD_WARNING
            || (ub as i128) > THRESHOLD_WARNING
            || (ub as i128) < -THRESHOLD_WARNING
        {
            tracing::warn!(
                "Variable {} in the LP has important bounds, may compromise LP stability",
                var.idx()
            );
        }
        self.bounds.push(lb, ub);
        debug_assert_eq!(var.idx(), self.bounds.current.len() - 1);

        stats.num_variables += 1;

        var
    }

    /// The range of the term `factor * var` over the initial bounds of `var` (which always hold)
    pub fn init_range_of_term(&self, factor: IntCst, var: Variable) -> (LongCst, LongCst) {
        let factor = cst_int_to_long(factor);
        let (lb, ub) = self.bounds.init[var.idx()];
        if factor >= 0 {
            (factor * lb, factor * ub)
        } else {
            (factor * ub, factor * lb)
        }
    }

    /// Sets a new upper or lower bound of the given variable.
    ///
    /// `justification` tells how the new bound was established (and it will be used to build the explanations involving it).
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
    ) -> Result<(), InfeasibilityReason> {
        debug_assert!(var.idx() < self.bounds.current.len());

        match bound {
            Bound::Lower => {
                self.bounds.current[var.idx()].lower = val;
                self.bounds.current[var.idx()].lower_justification = justification;
            }
            Bound::Upper => {
                self.bounds.current[var.idx()].upper = val;
                self.bounds.current[var.idx()].upper_justification = justification;
            }
        }

        self.lp.set_bound(var, bound, val as f64)
    }

    /// Same as [`Solver::set_bound_restrict`], but the change is *not* trailed:
    /// the bound will never be undone by a backtrack.
    ///
    /// Only valid for a `justification` that holds at the root, i.e. always / unconditionally.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediately detected as infeasible.
    pub fn set_bound_restrict_permanent(&mut self, c: &BoundRestriction) -> Result<bool, InfeasibilityReason> {
        let restricts = match c.bound {
            Bound::Lower => c.val > self.bounds.current[c.var.idx()].lower,
            Bound::Upper => c.val < self.bounds.current[c.var.idx()].upper,
        };
        if restricts {
            let unconditional = &mut self.bounds.unconditional[c.var.idx()];
            match c.bound {
                Bound::Lower => unconditional.0 = unconditional.0.max(c.val),
                Bound::Upper => unconditional.1 = unconditional.1.min(c.val),
            }
            self.set_bound(c.var, c.bound, c.val, BoundJustification::Unconditional)?;
        }
        Ok(restricts)
    }

    /// Sets a new upper or lower bound of the given variable, if it is tighter than the current one (trailed: undone on backtrack).
    /// Returns whether the bound was modified.
    ///
    /// # Errors
    ///
    /// Will return an error if the problem is immediately detected as infeasible.
    pub fn set_bound_restrict(
        &mut self,
        c: &BoundRestriction,
        justification: BoundJustification,
        trail: &mut Trail<LpEvent>,
    ) -> Result<bool, InfeasibilityReason> {
        let b = &self.bounds.current[c.var.idx()];
        let (old_val, old_justification, restricts) = match c.bound {
            Bound::Lower => (b.lower, b.lower_justification, c.val > b.lower),
            Bound::Upper => (b.upper, b.upper_justification, c.val < b.upper),
        };
        if !restricts {
            return Ok(false);
        }
        trail.push(LpEvent {
            var: c.var,
            bound: c.bound,
            old_val,
            old_justification,
        });
        self.set_bound(c.var, c.bound, c.val, justification)?;
        Ok(true)
    }

    /// Checks whether the current LP is feasible, within the given budget.
    pub fn check_feasibility(
        &mut self,
        deadline: Deadline,
    ) -> Result<aries_lp::results::FeasibilityStatus, aries_lp::results::SolverFailure> {
        self.lp.check_feasibility_with_deadline(deadline)
    }

    /// Adds a constraint `lin_sum = 0`, both to the LP and to its exact integer copy.
    pub fn add_constraint(&mut self, lin_sum: LpSum) {
        let float_lin_sum: Vec<(Variable, f64)> = lin_sum.iter().map(|(var, coef)| (var, coef as f64)).collect();

        self.lp.add_constraint(&float_lin_sum, aries_lp::ComparisonOp::Eq, 0.0);

        self.constraints.push(lin_sum);
    }

    /// The maximum of a linear sum (one coefficient per variable) over the current bounds, None on overflow.
    fn max_lin_sum(&self, lin_sum: &[i128]) -> Option<i128> {
        lin_sum.iter().enumerate().try_fold(0i128, |acc, (i, &coeff)| {
            if coeff == 0 {
                Some(acc)
            } else {
                let bound = if coeff < 0 {
                    self.bounds.current[i].lower as i128
                } else {
                    self.bounds.current[i].upper as i128
                };

                let prod = bound.checked_mul(coeff)?;

                acc.checked_add(prod)
            }
        })
    }

    /// The minimum of a linear sum (one coefficient per variable) over the current bounds, None on overflow.
    fn min_lin_sum(&self, lin_sum: &[i128]) -> Option<i128> {
        lin_sum.iter().enumerate().try_fold(0i128, |acc, (i, &coeff)| {
            if coeff == 0 {
                Some(acc)
            } else {
                let bound = if coeff > 0 {
                    self.bounds.current[i].lower as i128
                } else {
                    self.bounds.current[i].upper as i128
                };

                let prod = bound.checked_mul(coeff)?;

                acc.checked_add(prod)
            }
        })
    }

    /// Converts a certificate's f64 multipliers into i128 ones (scaled by 2^52, then truncated).
    fn convert_certificate_i128(cert: &[f64]) -> Vec<i128> {
        let coef = 2.0_f64.powi(52); // 52

        cert.iter().map(|x| (x * coef).trunc() as i128).collect()
    }

    /// Applies a certificate (i.e. its multipliers) to the solver's constraints (all of the form `sum = 0`).
    /// Returns the resulting sum's coefficients (for each variable), as well as its minimum and maximum over the current bounds.
    /// None on overflow.
    fn combine_constraints(&self, multipliers: &[i128]) -> Option<(Vec<i128>, i128, i128)> {
        let mut lin_sum: Vec<i128> = vec![0; self.bounds.current.len()];
        for (constraint, &mult) in self.constraints.iter().zip(multipliers) {
            if mult == 0 {
                continue;
            }
            for (var, coef) in constraint.iter() {
                let prod = mult.checked_mul(coef as i128)?;
                lin_sum[var.idx()] = lin_sum[var.idx()].checked_add(prod)?;
            }
        }
        let min = self.min_lin_sum(&lin_sum)?;
        let max = self.max_lin_sum(&lin_sum)?;
        Some((lin_sum, min, max))
    }

    /// Validates a certificate of infeasibility, with exact integer arithmetic.
    ///
    /// Returns None if the certificate isn't valid, otherwise the explanation of the infeasibility
    pub fn check_certificate(
        &self,
        cert: &[f64],
        domains: &DomainsSnapshot,
        stats: &mut Stats,
        bc_store: &BoundConstraintsStore,
    ) -> Option<Explanation> {
        debug_assert_eq!(cert.len(), self.constraints.len());
        let cert_i128 = Solver::convert_certificate_i128(cert);

        // This linear sum should be equal to 0: this is the certificate's corresponding aggregate constraint.
        // (As such, when this sum does not equal 0, it means we are at an infeasibility).
        let Some((lin_sum, min_lin_sum, max_lin_sum)) = self.combine_constraints(&cert_i128) else {
            stats.num_overflow += 1;
            return None;
        };

        // To detect the infeasibility, we check whether 0 is outside the [min, max] range, as our linear sum should be equal to 0 (see above).
        if max_lin_sum < 0 {
            let opposite = lin_sum.iter().map(|&coef| -coef).collect_vec();
            return Some(self.explain_leq(&opposite, domains, bc_store));
        }
        if min_lin_sum > 0 {
            return Some(self.explain_leq(&lin_sum, domains, bc_store));
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

    /// Explains why the current bounds of the LP violate `sum_j lin_sum[j] * x_j <= 0`,
    /// with one coefficient per LP variable in `lin_sum` (0 for the variables not actually in the sum).
    ///
    /// The sum's minimum is above 0: each term `lin_sum[j] * x_j` takes the bound of `x_j` that minimizes it
    /// (its lower bound for a positive coefficient, its upper bound for a negative one).
    ///
    /// The explanation is made of the literals that justify these bounds (see [`BoundJustification`]):
    /// - A bound from the model is justified by a literal of the model.
    ///   When refined, this literal is weakened as long as the minimum stays above 0, as in [`crate::reasoners::cp::linear`]:
    ///   e.g. `x <= 8` rather than the current `x <= 5`, if `x <= 8` already suffices.
    ///   This takes a bit more time, but results in a more general clause.
    /// - A bound from a bound constraint is justified by the literals of its trigger.
    ///   This is how the LP variables without a model variable (auxiliary and reification variables) contribute to the explanation.
    ///
    /// Unconditional bounds (initial, or from a trigger that holds at the root) do not need to participate in the explanation.
    fn explain_leq(
        &self,
        lin_sum: &[i128],
        domains: &DomainsSnapshot,
        bc_store: &BoundConstraintsStore,
    ) -> Explanation {
        // We iterate over the lin_sum to eliminate variables with a null coef
        // Variables that are really present in the constraint (coef != 0) are then treated separately
        // depending on how their minimizing bound was established, as recorded in its justification.
        let terms = lin_sum.iter().enumerate().filter_map(|(idx, &coef)| {
            // Bound of the LP variable in the direction that minimizes the sum
            // (lower bound for a positive coefficient, upper bound for a negative one)
            // together with the justification recorded when it was set.
            let b = &self.bounds.current[idx];
            let bound = if coef > 0 {
                (b.lower, b.lower_justification)
            } else if coef < 0 {
                (b.upper, b.upper_justification)
            } else {
                return None;
            };
            Some((idx, coef, bound))
        });
        self.explain_violation(terms, domains, bc_store)
    }

    /// Explains why `sum_j a_j x_j <= 0` is violated, i.e. why `sum_j a_j x_j` is strictly above 0.
    /// Each term is given as `(j, a_j, bound)`, where `bound` is the bound of `x_j` that minimizes `a_j x_j`, with its justification.
    ///
    /// The literals are chosen from the justifications as described for [`Self::explain_leq`].
    fn explain_violation(
        &self,
        terms: impl IntoIterator<Item = (usize, i128, (LongCst, BoundJustification))>,
        domains: &DomainsSnapshot,
        bc_store: &BoundConstraintsStore,
    ) -> Explanation {
        let mut explanation = Explanation::new();

        if !self.is_explanation_refined {
            for (_, _, (_, justification)) in terms {
                justification.push_to(&mut explanation, bc_store);
            }
            return explanation;
        }

        // We always explain a contradiction, i.e. lower bounds summing to *strictly* more than 0,
        // so the budget is raised by one to match the non-strict comparisons used below.
        // Same trick as in [`crate::reasoners::cp::linear::explain`].
        let mut ub = 1;

        let mut culprits = BinaryHeap::new();

        for (idx, coef, (bound, justification)) in terms {
            debug_assert!(coef != 0);

            match justification {
                // The bound was pushed by the synchronization with the main model, which entails it:
                // the events of the main model's history justify it and can be minimized.
                // The synchronization only concerns variables that are mapped to a var in aries solver.
                BoundJustification::CpModel(_) => {
                    let &var = self
                        .lp_to_model
                        .get(idx)
                        .expect("a bound entailed by the main model can only be set on a variable mapped to it");
                    let sum_elem = SumElem::new(coef, var);

                    if let Some(event) = LbBoundEvent::new(sum_elem, domains) {
                        // there is a lower bound event on this element, add it to the set of culprits for later processing
                        culprits.push(event)
                    } else {
                        // no event associated to the element, which means its value is entailed at the ROOT.
                        // Hence it does not need to be in the explanation, but its contribution cancels from the UB
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
                    // there was no previous event (i.e. the previous lower bound always holds)
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

    /// Explains the incompatible bounds of a variable (its lower bound above its upper bound): the justifications of both.
    pub fn explain_infeasible_var(&self, var: Variable, bc_store: &BoundConstraintsStore) -> Explanation {
        let mut explanation = Explanation::new();

        let int_bound = &self.bounds.current[var.idx()];

        for justification in [int_bound.upper_justification, int_bound.lower_justification] {
            justification.push_to(&mut explanation, bc_store);
        }

        explanation
    }
}
