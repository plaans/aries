use std::sync::Arc;

use crate::{
    collections::ref_store::RefVec,
    core::{state::ValueCause, views::Dom},
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
    pub(super) fn new(values: RefVec<SignedVar, ValueCause>, presences: RefVec<VarRef, Lit>) -> Self {
        Self {
            data: Arc::new(SolutionInternal { values, presences }),
        }
    }

    /// Returns the number of variables declared.
    pub fn num_variables(&self) -> usize {
        debug_assert!(self.data.values.len().is_multiple_of(2));
        self.data.values.len() / 2
    }

    /// Returns all variables.
    pub fn variables(&self) -> impl Iterator<Item = VarRef> {
        (0..self.num_variables()).map(VarRef::from)
    }

    /// Returns all variables whose value is fixed.
    pub fn bound_variables(&self) -> impl Iterator<Item = (VarRef, IntCst)> + '_ {
        self.variables().filter_map(move |v| {
            let lb = self.lb(v);
            let ub = self.ub(v);
            if lb == ub { Some((v, lb)) } else { None }
        })
    }
}

pub(super) struct SolutionInternal {
    pub(super) values: RefVec<SignedVar, ValueCause>,
    pub(super) presences: RefVec<VarRef, Lit>,
}

impl Dom for Solution {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.data.values[svar].upper_bound
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.data.presences[var]
    }
}
