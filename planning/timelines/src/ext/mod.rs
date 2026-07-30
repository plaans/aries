pub mod ground;
mod nonsimple;

use std::ops::Index;

use aries_solver::core::{INT_CST_MAX, IntCst, LongCst};

pub use nonsimple::collect_nonsimple_conditions_and_effects_to_relax;

pub(crate) type Source = Option<crate::TaskId>;

pub type GroundingFlatId = Option<usize>;
/// A wrapper around a vector of constants.
/// Can be flattened into a integer id given the first value and dimension of each "column".
/// In practice, these come from the integer encoding ranges of state functions' parameter types.
#[derive(Debug, Clone)]
pub struct Grounding(Vec<IntCst>);

impl Grounding {
    fn to_flat_id(&self, ranges: &[(IntCst, IntCst)]) -> GroundingFlatId {
        debug_assert!(self.0.len() == ranges.len());

        if self.0.is_empty() {
            return None;
        }

        let mut res = 0;
        let mut factor = 1;
        for (&n, &(lb, ub)) in self.0.iter().zip(ranges).rev() {
            debug_assert!((ub as LongCst - lb as LongCst) + 1 >= 0, "{lb} {ub}");
            debug_assert!((ub as LongCst - lb as LongCst) < INT_CST_MAX as LongCst, "{lb} {ub}");
            let (first, dim) = (lb, usize::try_from(ub - lb + 1).unwrap());

            debug_assert!((n as LongCst - first as LongCst) >= 0, "{n} {first}");
            debug_assert!(
                (n as LongCst - first as LongCst) <= INT_CST_MAX as LongCst,
                "{n} {first}"
            );
            debug_assert!(usize::try_from(n - first).unwrap() <= dim, "{n} {first}");

            res += usize::try_from(n - first).unwrap() * factor;
            factor *= dim;
        }
        Some(res)
    }
}
impl Index<usize> for Grounding {
    type Output = IntCst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
