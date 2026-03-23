use anyhow::Result;
use anyhow::ensure;

use crate::fzn::types::Int;

/// Generic range defined by its two bounds.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Range<T: PartialOrd> {
    lb: T,
    ub: T,
}

impl<T: PartialOrd> Range<T> {
    /// Create a `Range` with the given bounds.
    ///
    /// Return an `Error` if the lower bound is greater than the upper bound.
    pub fn new(lb: T, ub: T) -> Result<Range<T>> {
        ensure!(lb <= ub, "lb is greater than ub");
        let range = Range { lb, ub };
        Ok(range)
    }

    /// Return the range lower bound.
    pub fn lb(&self) -> &T {
        &self.lb
    }

    /// Return the range upper bound.
    pub fn ub(&self) -> &T {
        &self.ub
    }

    /// Return both lower and upper bounds.
    pub fn bounds(&self) -> (&T, &T) {
        (&self.lb, &self.ub)
    }
}

/// Integer range.
///
/// ```flatzinc
/// var 1..9: x;
/// ```
pub type IntRange = Range<Int>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getters() {
        let attrs_ok = [(-2, 1, true), (1, 1, true), (3, 2, false)];
        for (lb, ub, ok) in attrs_ok {
            let var = Range::new(lb, ub);
            if ok {
                let var = var.expect("var should be Ok");
                assert_eq!(*var.lb(), lb);
                assert_eq!(*var.ub(), ub);
                assert_eq!(var.bounds(), (&lb, &ub));
            } else {
                assert!(var.is_err());
            }
        }
    }
}
