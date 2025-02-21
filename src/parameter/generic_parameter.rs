use crate::types::Int;


#[derive(Eq, Hash, Debug)]
pub struct GenericParameter<T> {
    value: T,
}

impl<T> GenericParameter<T> {
    /// Return a new `GenericParameter` with the given value.
    pub fn new(value: T) -> Self {
        GenericParameter { value }
    }

    /// Return the parameter value.
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> PartialEq for GenericParameter<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

pub type IntParameter = GenericParameter<Int>;
pub type BoolParameter = GenericParameter<bool>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        let x = IntParameter::new(9);
        let y = IntParameter::new(9);

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}