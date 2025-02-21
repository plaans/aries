use std::hash::Hash;

use crate::traits::Identifiable;
use crate::types::Id;
use crate::types::Int;


#[derive(Eq, Hash, Debug)]
pub struct GenericParameter<T> {
    id: Id,
    value: T,
}

impl<T> GenericParameter<T> {
    /// Return a new `GenericParameter` with the given id and value.
    pub fn new(id: Id, value: T) -> Self {
        GenericParameter { id, value }
    }

    /// Return the parameter value.
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> Identifiable for GenericParameter<T> {
    fn id(&self) -> &Id {
        &self.id
    }
}

// Remark: PartialEq is only needed for the debug assert
impl<T: PartialEq> PartialEq for GenericParameter<T> {
    fn eq(&self, other: &Self) -> bool {
        debug_assert!(
            self.id != other.id || self.value == other.value,
            "same id but different domains",
        );
        self.id == other.id
    }
}

pub type IntParameter = GenericParameter<Int>;
pub type BoolParameter = GenericParameter<bool>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        let x = IntParameter::new("x".to_string(), 9);
        let y = IntParameter::new("y".to_string(), 9);

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}