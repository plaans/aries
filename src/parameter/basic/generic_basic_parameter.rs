use std::hash::Hash;

use crate::traits::Identifiable;
use crate::types::Id;
use crate::types::Int;


#[derive(Eq, Hash, Debug)]
pub struct GenericBasicParameter<T> {
    id: Id,
    value: T,
}

impl<T> GenericBasicParameter<T> {
    /// Return a new `GenericParameter` with the given id and value.
    pub(crate) fn new(id: Id, value: T) -> Self {
        GenericBasicParameter { id, value }
    }

    /// Return the parameter value.
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> Identifiable for GenericBasicParameter<T> {
    fn id(&self) -> &Id {
        &self.id
    }
}

// Remark: PartialEq is only needed for the debug assert
impl<T: PartialEq> PartialEq for GenericBasicParameter<T> {
    fn eq(&self, other: &Self) -> bool {
        debug_assert!(
            self.id != other.id || self.value == other.value,
            "same id but different domains",
        );
        self.id == other.id
    }
}

pub type IntParameter = GenericBasicParameter<Int>;
pub type BoolParameter = GenericBasicParameter<bool>;

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