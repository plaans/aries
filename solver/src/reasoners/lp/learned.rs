//! Theory propagation of the LP reasoner from its past infeasibility certificates, without the simplex.
//!
//! A certificate's multipliers combine the LP constraints, which are all equalities (`sum = 0`),
//! into an equation `sum_j a_j x_j = 0`, valid in every state.
//! It is learned as two constraints, `sum_j a_j x_j <= 0` and `sum_j -a_j x_j <= 0` (see [`CertAggregateConstraint`]).
//! A learned constraint is checked again after an event on its variables, in the model's current state (the LP may lag behind):
//!  - if its minimum is above 0, it is violated: a contradiction;
//!  - otherwise, its "threats" are deactivated. A threat is a bound constraint on one of its variables whose bound
//!    would raise the minimum above 0, so the threat's trigger can't hold:
//!    once all the literals of the trigger but one are set, the last one is set false (unit propagation).
//!
//! For example, with the learned constraint `x - y <= 0` and the bound `y <= 3`,
//! the bound constraint `l1 & l2 -> x >= 5` is a threat, since `x >= 5` would raise the minimum to `5 - 3 = 2`.
//! Once `l1` is set, `l2` is set false.

use aries_lp::prelude::*;

use crate::backtrack::ObsTrailCursor;
use crate::collections::set::IterableRefSet;
use crate::core::literals::Conjunction;
use crate::core::state::{Cause, Domains, DomainsSnapshot, Event, Explanation};
use crate::core::views::Dom;
use crate::core::{Lit, LongCst, cst_int_to_long};
use crate::reasoners::{Contradiction, ReasonerId};

use super::{BoundConstraintId, BoundConstraintsStore, BoundJustification, Solver};

/// A learned constraint `sum_j a_j x_j <= 0`: one side of a certificate's equation, violated once its minimum is above 0.
#[derive(Clone)]
pub(super) struct CertAggregateConstraint {
    terms: Vec<(u32, i128)>,
}

impl CertAggregateConstraint {
    /// The constraint of the non-null coefficients of a linear sum (one coefficient per variable).
    pub(super) fn new(lin_sum: &[i128]) -> Self {
        let terms = lin_sum
            .iter()
            .enumerate()
            .filter(|&(_, &a)| a != 0)
            .map(|(j, &a)| (j as u32, a))
            .collect();
        CertAggregateConstraint { terms }
    }

    /// The bounds that its minimum takes in a state, with their justification, and the minimum.
    fn min(
        &self,
        doms: &impl Dom,
        solver: &Solver,
        bc_store: &BoundConstraintsStore,
    ) -> (Vec<(LongCst, BoundJustification)>, i128) {
        let bounds: Vec<_> = self
            .terms
            .iter()
            .map(|&(j, a)| var_bound_in(doms, j as usize, a > 0, solver, bc_store))
            .collect();
        let min = self
            .terms
            .iter()
            .zip(&bounds)
            .map(|(&(_, a), &(v, _))| a * v as i128)
            .sum();
        (bounds, min)
    }

    /// The "threats" to the constraint, given the `bounds` that its minimum `min` takes in a state.
    /// The threats are the bound constraints on its variables whose bound would raise the minimum above 0, each with the index of its term.
    fn threats<'a>(
        &'a self,
        bounds: &'a [(LongCst, BoundJustification)],
        min: i128,
        solver: &'a Solver,
        bc_store: &'a BoundConstraintsStore,
    ) -> impl Iterator<Item = (usize, BoundConstraintId)> + 'a {
        self.terms.iter().enumerate().flat_map(move |(k, &(j, a))| {
            bc_store.constrs_on(j as usize).iter().filter_map(move |&id| {
                let restr = &bc_store.store[id].1;
                // only the bound that the minimum takes can raise it: the lower bound if `a > 0`, the upper bound otherwise
                if matches!(restr.bound, Bound::Lower) != (a > 0) {
                    return None;
                }
                // the minimum, with the bound constraint's bound in place of the current one
                let new_min = min + a * (clamped(restr.val, j as usize, solver) as i128 - bounds[k].0 as i128);
                (new_min > 0).then_some((k, id))
            })
        })
    }
}

/// The constraints learned from the infeasibility certificates.
///
/// NOTE TODO: For now, they are never dropped. But later, those that have not taken part in a contradiction or a deactivation for a long while could be dropped.
#[derive(Clone, Default)]
pub(super) struct LearnedConstraints {
    constrs: Vec<CertAggregateConstraint>,
    /// For each variable, the learned constraints it appears in.
    by_var: Vec<Vec<u32>>,
    /// The not yet read events of the model, used to know which learned constraints to check.
    /// It is separate from the LP's cursor (`Lp::model_events`) which only reads the events up to the LP's level (which may be lagging behind).
    /// Since the learned constraints are checked at the model's current level, even when the LP lags behind, this new cursor is needed.
    events: ObsTrailCursor<Event>,
    /// The learned constraints to check at the next propagation (all of them when `all_pending` is true).
    pending: IterableRefSet<usize>,
    all_pending: bool,

    /// Stats: number of rejected (not learned) certificates
    /// (due to the minima of their constraints possibly overflowing `i128`)
    pub(super) num_rejected: usize,
    /// Stats: number of contradictions found by the learned constraints
    pub(super) num_contradictions: usize,
    /// Stats: number of literals set by deactivations
    pub(super) num_deactivations: usize,
}

impl LearnedConstraints {
    /// Learns `cert_aggregate` (`sum_j a_j x_j <= 0`) and its opposite (`sum_j -a_j x_j <= 0`),
    /// which both hold since the certificate's sum must be equal to 0 in every state (see the module doc).
    /// Nothing is learned if their minima could overflow `i128`.
    pub(super) fn learn(&mut self, cert_aggregate: &CertAggregateConstraint, solver: &Solver) {
        let init = &solver.bounds.init;

        // the minima stay within sum_j |a_j| |b_j|, over the largest bounds b_j the variables can take:
        // their initial bounds, to which all the bounds used are clamped
        let magnitude = cert_aggregate.terms.iter().try_fold(0i128, |acc, &(j, a)| {
            let (lb, ub) = init[j as usize];
            let bound = (lb as i128).abs().max((ub as i128).abs());

            acc.checked_add(a.checked_abs()?.checked_mul(bound)?)
        });
        if magnitude.is_none_or(|m| m >= i128::MAX / 4) {
            self.num_rejected += 1;
            return;
        }

        self.by_var.resize(init.len(), Vec::new());
        for sign in [1, -1] {
            let i = self.constrs.len();
            for &(j, _) in &cert_aggregate.terms {
                self.by_var[j as usize].push(i as u32);
            }
            let terms = cert_aggregate.terms.iter().map(|&(j, a)| (j, sign * a)).collect();

            self.constrs.push(CertAggregateConstraint { terms });
            self.pending.insert(i);
        }
    }

    /// Marks all the learned constraints as pending, for the case where the model events aren't enough to tell which learned constraints to check.
    /// This happens after a backtrack (constraints learned at a deeper level were never checked at the level backtracked to),
    /// or after a new bound constraint is introduced (its trigger may already hold, with no event left to read).
    pub(super) fn mark_pending_all(&mut self) {
        self.all_pending = true;
    }

    /// Marks as pending the learned constraints that the events since the previous call may affect, those on:
    ///  - the mapped variable of an event (its bound changed),
    ///  - the variable of a bound constraint with a literal of its trigger set by an event (the trigger may now hold, or lack a single literal).
    fn mark_pending(&mut self, doms: &Domains, solver: &Solver, bc_store: &BoundConstraintsStore) {
        if self.all_pending {
            // all learned constraints will be checked, so the events so far can be skipped
            self.events.move_to_end(doms.trail());

            for i in 0..self.constrs.len() {
                self.pending.insert(i);
            }
            self.all_pending = false;
        }
        while let Some(event) = self.events.pop(doms.trail()) {
            let mapped = solver.model_to_lp.get(event.affected_bound.variable()).map(|v| v.idx());
            let constrained = bc_store
                .watches_on(event.new_literal())
                .map(|id| bc_store.store[id].1.var.idx());

            for var in mapped.into_iter().chain(constrained) {
                for &i in self.by_var.get(var).into_iter().flatten() {
                    self.pending.insert(i as usize);
                }
            }
        }
    }

    /// Checks the pending learned constraints in the model's current state:
    /// a violated one is a contradiction, otherwise its threats are deactivated (see the module doc).
    /// Returns whether some literal was set.
    pub(super) fn propagate(
        &mut self,
        doms: &mut Domains,
        id: ReasonerId,
        solver: &Solver,
        bc_store: &BoundConstraintsStore,
    ) -> Result<bool, Contradiction> {
        if self.constrs.is_empty() {
            return Ok(false);
        }
        self.mark_pending(doms, solver, bc_store);

        let mut num_set = 0;
        for i in self.pending.iter() {
            let constr = &self.constrs[i];
            let (bounds, min) = constr.min(doms, solver, bc_store);

            if min > 0 {
                self.num_contradictions += 1;

                let terms = constr.terms.iter().zip(bounds).map(|(&(j, a), b)| (j as usize, a, b));
                let explanation = solver.explain_violation(terms, 0, &DomainsSnapshot::current(doms), bc_store);

                return Err(Contradiction::Explanation(explanation));
            }
            for (_, threat) in constr.threats(&bounds, min, solver, bc_store) {
                if let Some(last) = last_unset(&bc_store.store[threat].0, doms) {
                    num_set += doms.set(!last, Cause::inference(id, i as u32))? as usize;
                }
            }
        }
        self.pending.clear();
        self.num_deactivations += num_set;

        Ok(num_set > 0)
    }

    /// Explains a literal set by the deactivation of a threat to the learned constraint `constr`, in the `state` it was set from.
    /// The explanation is composed of the literals of the threat's trigger (except the one set false, i.e. whose negation is the literal to explain),
    /// and of literals that bound the other variables of the constraint tightly enough for the threat's bound to violate the learned constraint.
    pub(super) fn explain_deactivation(
        &self,
        literal: Lit,
        constr: u32,
        state: &DomainsSnapshot,
        out: &mut Explanation,
        solver: &Solver,
        bc_store: &BoundConstraintsStore,
    ) {
        let constr = &self.constrs[constr as usize];
        let (bounds, min) = constr.min(state, solver, bc_store);

        // the deactivation that set the literal (the solver may ask for a weaker literal than the one set)
        let (k, threat, last) = constr
            .threats(&bounds, min, solver, bc_store)
            .find_map(|(k, threat)| {
                let last = last_unset(&bc_store.store[threat].0, state)?;
                (!last).entails(literal).then_some((k, threat, last))
            })
            .expect("a threat of the learned constraint sets the literal");

        // were `last` true, the threat's term would be at least `fixed` (with the threat's bound),
        // and the bounds of the other terms would violate `(sum of the other terms) + fixed <= 0`
        let (trigger, restr) = &bc_store.store[threat];
        let (j, a) = constr.terms[k];
        let fixed = a * clamped(restr.val, j as usize, solver) as i128;
        let others = (constr.terms.iter().zip(bounds).enumerate())
            .filter(|&(i, _)| i != k)
            .map(|(_, (&(j, a), b))| (j as usize, a, b));

        out.extend(
            solver
                .explain_violation(others, fixed, state, bc_store)
                .literals()
                .iter()
                .copied(),
        );
        out.extend(trigger.iter().filter(|&l| l != last));
    }

    pub(super) fn num_constrs(&self) -> usize {
        self.constrs.len()
    }

    pub(super) fn print_stats(&self) {
        println!(
            "# learned constraints: {}, rejected certificates: {}, contradictions: {}, deactivations: {}",
            self.num_constrs(),
            self.num_rejected,
            self.num_contradictions,
            self.num_deactivations
        );
    }
}

/// The lower bound (if `lower`) or upper bound of an LP variable in a state of the model, with its justification.
/// It is the tightest of:
///  - the variable's unconditional bound,
///  - the bounds of the bound constraints on it whose trigger holds,
///  - the bound of its mapped model variable, if any.
fn var_bound_in(
    doms: &impl Dom,
    var: usize,
    lower: bool,
    solver: &Solver,
    bc_store: &BoundConstraintsStore,
) -> (LongCst, BoundJustification) {
    let (lb, ub) = solver.bounds.unconditional[var];
    let mut best = (if lower { lb } else { ub }, BoundJustification::Unconditional);
    let tighter = |v: LongCst, cur: LongCst| if lower { v > cur } else { v < cur };

    for &id in bc_store.constrs_on(var) {
        let (trigger, restr) = &bc_store.store[id];
        if matches!(restr.bound, Bound::Lower) == lower
            && tighter(restr.val, best.0)
            && trigger.iter().all(|l| doms.entails(l))
        {
            best = (restr.val, BoundJustification::Trigger(id));
        }
    }
    if let Some(&model_var) = solver.lp_to_model.get(var) {
        let (val, lit) = if lower {
            let v = doms.lb(model_var);
            (cst_int_to_long(v), Lit::geq(model_var, v))
        } else {
            let v = doms.ub(model_var);
            (cst_int_to_long(v), Lit::leq(model_var, v))
        };
        if tighter(val, best.0) {
            best = (val, BoundJustification::CpModel(lit));
        }
    }
    (clamped(best.0, var, solver), best.1)
}

/// Clamps a bound of a variable to its initial bounds.
fn clamped(val: LongCst, var: usize, solver: &Solver) -> LongCst {
    let (lb, ub) = solver.bounds.init[var];
    val.max(lb).min(ub)
}

/// The only literal of a trigger that is not entailed in a state, if there is exactly one.
fn last_unset(trigger: &Conjunction, doms: &impl Dom) -> Option<Lit> {
    let mut unset = trigger.iter().filter(|&l| !doms.entails(l));
    match (unset.next(), unset.next()) {
        (Some(last), None) => Some(last),
        _ => None,
    }
}
