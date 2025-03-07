use std::hash::Hash;

use crate::traits::Identifiable;
use crate::types::Id;
use crate::types::Int;


#[derive(Eq, Hash, Debug)]
pub struct GenPar<T> {
    id: Id,
    value: T,
}

impl<T> GenPar<T> {
    /// Return a new `GenPar` with the given id and value.
    pub(crate) fn new(id: Id, value: T) -> Self {
        GenPar { id, value }
    }

    /// Return the parameter value.
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> Identifiable for GenPar<T> {
    fn id(&self) -> &Id {
        &self.id
    }
}

// Remark: PartialEq is only needed for the debug assert
impl<T: PartialEq> PartialEq for GenPar<T> {
    fn eq(&self, other: &Self) -> bool {
        debug_assert!(
            self.id != other.id || self.value == other.value,
            "same id but different domains",
        );
        self.id == other.id
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