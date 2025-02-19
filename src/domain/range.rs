use anyhow::{ensure, Result};

use crate::types::*;

/// Generic range defined by its two bounds.
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
}

pub type IntRange = Range<Int>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_getters() {
        let attrs_ok = [
            (-2, 1,  true),
            ( 1, 1,  true),
            ( 3, 2, false),
        ];
        for (lb, ub, ok) in attrs_ok {
            let var = Range::new(lb, ub);
            if ok {
                let var = var.expect("result should be Ok");
                assert_eq!(*var.lb(), lb);
                assert_eq!(*var.ub(), ub);
            } else {
                assert!(var.is_err());
            }
        }
    }
}