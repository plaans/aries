use smallvec::SmallVec;

use crate::{core::IntCst, reasoners::lp::LpVar};

/// Represents a sum of scaled LP variables: `var_1 * factor_1 + var_2 * factor_2 + ...`.
///
/// The type maintains a normalized representation (order, deduplication), to facilitate comparison.
#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct LpSum {
    terms: SmallVec<[LpLinTerm; 2]>,
}

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
struct LpLinTerm {
    var: LpVar,
    factor: IntCst,
}

impl LpSum {
    pub fn new(terms: impl IntoIterator<Item = (LpVar, IntCst)>) -> Self {
        let mut this = Self {
            terms: terms
                .into_iter()
                .map(|(var, factor)| LpLinTerm { var, factor })
                .collect(),
        };
        this.simplify();
        this
    }

    pub fn push(&mut self, var: LpVar, factor: IntCst) {
        self.terms.push(LpLinTerm { var, factor });
        self.simplify();
    }

    pub fn iter(&self) -> impl Iterator<Item = (LpVar, IntCst)> {
        self.terms.iter().map(|t| (t.var, t.factor))
    }

    /// If the sum contains a single term, returns it.
    pub(super) fn as_single_term(&self) -> Option<(LpVar, IntCst)> {
        self.terms.as_array().map(|&[t]| (t.var, t.factor))
    }

    /// Simplify the terms of the expression, into a normalized expression that satisfies
    /// all our invariants.
    ///
    /// Note that it should be an invariant that the `LinSum` is always in its normal form.
    ///
    /// This is private because no-one outside this module should be able to hold a `LinSum` that is not normalized already.
    fn simplify(&mut self) {
        self.terms.sort_unstable_by_key(|sv| sv.var);
        self.terms.dedup_by(|second, first| {
            if first.var == second.var {
                // same variables, merge
                first.factor += second.factor;
                true // remove second
            } else {
                false // different vars, don't merge
            }
        });
        self.terms.retain(|sv| sv.factor != 0);
    }
}

impl<T: IntoIterator<Item = (LpVar, IntCst)>> From<T> for LpSum {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
