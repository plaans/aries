
#[non_exhaustive]
#[derive(Clone, Eq, Hash, Debug)]
pub struct BoolVariable;

impl BoolVariable {
    /// Create a new `BoolVariable`.
    pub fn new() -> Self {
        BoolVariable
    }
}

impl PartialEq for BoolVariable {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        let x = BoolVariable;
        let y = BoolVariable::new();

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}