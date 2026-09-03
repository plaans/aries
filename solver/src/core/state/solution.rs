use std::sync::Arc;

use crate::{
    collections::ref_store::RefVec,
    core::{
        state::ValueCause,
        views::{Dom, Optional, VarView},
    },
    prelude::*,
};

/// Represents a solution of the problem, i.e., a copy of the domains of the variables.
///
/// It is designed primarily to be:
///  - efficient to create from an existing domains (just two vector to memcopy)
///  - very cheap to copy (data behind a ref counted)
///
/// It can be created with [`crate::core::state::Domains::extract_solution`].
#[derive(Clone)]
pub struct Solution {
    pub(super) data: Arc<SolutionInternal>,
}

impl Solution {
    pub(super) fn new(values: RefVec<SignedVar, ValueCause>, presences: RefVec<Var, Lit>) -> Self {
        Self {
            data: Arc::new(SolutionInternal { values, presences }),
        }
    }

    /// Returns the value that the expression has in the solution, or None if the value is absent.
    pub fn eval<T: Evaluable>(&self, expr: T) -> Option<T::Value> {
        expr.evaluate(self)
    }

    /// Returns the number of variables declared.
    pub fn num_variables(&self) -> usize {
        debug_assert!(self.data.values.len().is_multiple_of(2));
        self.data.values.len() / 2
    }

    /// Returns all variables.
    pub fn variables(&self) -> impl Iterator<Item = Var> {
        (0..self.num_variables()).map(Var::from)
    }

    /// Returns all variables whose value is fixed.
    pub fn bound_variables(&self) -> impl Iterator<Item = (Var, IntCst)> + '_ {
        self.variables().filter_map(move |v| {
            let lb = self.lb(v);
            let ub = self.ub(v);
            if lb == ub { Some((v, lb)) } else { None }
        })
    }
}

pub(super) struct SolutionInternal {
    pub(super) values: RefVec<SignedVar, ValueCause>,
    pub(super) presences: RefVec<Var, Lit>,
}

impl Dom for Solution {
    fn _upper_bound(&self, svar: SignedVar) -> IntCst {
        self.data.values[svar].upper_bound
    }

    fn _presence(&self, var: Var) -> Lit {
        self.data.presences[var]
    }
}

/// Denotes expressions that can be evaluated in a solution.
pub trait Evaluable {
    /// Type of the value of this expression.
    ///
    /// For instance it would be [`IntCst`] for [`Var`] and `bool` for [`Lit`].
    type Value;

    /// Determines the value that the expression has in a solution.
    ///
    /// Returns `None`  the value is absent and the value wrapped in `Some(...)` otherwise.
    fn evaluate(&self, solution: &Solution) -> Option<Self::Value>;
}

impl<T> Evaluable for T
where
    T: VarView + Optional,
{
    type Value = <T as VarView>::Value;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if self.present(solution) {
            // in a solution it is guaranteed that the domain of any present variable is a singleton,
            // so we only take the lower bound
            Some(solution.lb(self))
        } else {
            None
        }
    }
}
