use std::hash::Hash;

use crate::types::Int;


#[derive(Eq, Hash, Debug)]
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

pub type ParBool = GenPar<bool>;
pub type ParInt = GenPar<Int>;

pub type ParBoolArray = GenPar<Vec<bool>>;
pub type ParIntArray = GenPar<Vec<Int>>;


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