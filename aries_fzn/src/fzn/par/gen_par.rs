use std::hash::Hash;

use crate::fzn::Fzn;
use crate::fzn::types::Int;

#[derive(Eq, Debug)]
pub struct GenPar<T> {
    name: String,
    value: T,
}

impl<T> GenPar<T> {
    /// Return a new `GenPar` with the given name and value.
    pub(crate) fn new(name: String, value: T) -> Self {
        GenPar { name, value }
    }

    /// Return the parameter name.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Return the parameter value.
    pub fn value(&self) -> &T {
        &self.value
    }
}

// Remark: PartialEq is only needed for the debug assert
impl<T: PartialEq> PartialEq for GenPar<T> {
    fn eq(&self, other: &Self) -> bool {
        debug_assert!(
            self.name != other.name || self.value == other.value,
            "same name but different values",
        );
        self.name == other.name
    }
}

// Remark: PartialEq is only needed for the debug assert
impl<T> Hash for GenPar<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

/// Boolean parameter.
///
/// ```flatzinc
/// bool: p = true;
/// ```
pub type ParBool = GenPar<bool>;

/// Integer parameter.
///
/// ```flatzinc
/// int: p = 3;
/// ```
pub type ParInt = GenPar<Int>;

/// Boolean array parameter.
///
/// ```flatzinc
/// array [1..2] of bool: p = [true, false];
/// ```
pub type ParBoolArray = GenPar<Vec<bool>>;

/// Integer array parameter.
///
/// ```flatzinc
/// array [1..3] of int: p = [2, 4, 8];
/// ```
pub type ParIntArray = GenPar<Vec<Int>>;

impl Fzn for ParBool {
    fn fzn(&self) -> String {
        format!("bool: {} = {};\n", self.name, self.value.fzn())
    }
}

impl Fzn for ParInt {
    fn fzn(&self) -> String {
        format!("int: {} = {};\n", self.name, self.value.fzn())
    }
}

impl Fzn for ParBoolArray {
    fn fzn(&self) -> String {
        format!(
            "array [1..{}] of bool: {} = {};\n",
            self.value.len(),
            self.name,
            self.value.fzn()
        )
    }
}

impl Fzn for ParIntArray {
    fn fzn(&self) -> String {
        format!(
            "array [1..{}] of int: {} = {};\n",
            self.value.len(),
            self.name,
            self.value.fzn()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        let x = ParInt::new("x".to_string(), 9);
        let y = ParInt::new("y".to_string(), 9);

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}
