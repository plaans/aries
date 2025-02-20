use crate::domain::IntDomain;

#[derive(Clone, Eq, Hash, Debug)]
pub struct IntVariable {
    domain: IntDomain,
}

impl IntVariable {
    /// Create a new bounded on the given domain.
    pub fn new(domain: IntDomain) -> Self {
        IntVariable { domain }
    }

    /// Return the domain of the given variable.
    pub fn domain(&self) -> &IntDomain {
        &self.domain
    }
}

impl PartialEq for IntVariable {
    /// A variable equal another iff it is the same object in memory.
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::IntRange;

    use super::*;

    #[test]
    fn clone() {
        let range = IntRange::new(-3, 5).unwrap();
        let domain = IntDomain::IntRange(range);

        let x = IntVariable::new(domain);
        let y = x.clone();

        assert_eq!(x.domain(), y.domain());
    }

    #[test]
    fn equality() {
        let range = IntRange::new(1, 3).unwrap();
        let domain = IntDomain::IntRange(range);

        let x = IntVariable::new(domain);
        let y = x.clone();

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}
