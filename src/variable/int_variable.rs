use crate::domain::IntDomain;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(Clone, Eq, Hash, Debug)]
pub struct IntVariable {
    id: Id,
    domain: IntDomain,
}

impl IntVariable {
    /// Create a new `IntVariable` with the given id and domain.
    pub fn new(id: Id, domain: IntDomain) -> Self {
        IntVariable { id, domain }
    }

    /// Return the variable domain.
    pub fn domain(&self) -> &IntDomain {
        &self.domain
    }
}

impl Identifiable for IntVariable {
    fn id(&self) -> &Id {
        &self.id
    }
}

impl PartialEq for IntVariable {
    fn eq(&self, other: &Self) -> bool {
        debug_assert!(
            self.id != other.id || self.domain == other.domain,
            "same id but different domains",
        );
        self.id == other.id
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::IntRange;

    use super::*;

    #[test]
    fn equality() {
        let range = IntRange::new(1, 3).unwrap();
        let domain = IntDomain::from(range);

        let x = IntVariable::new("x".to_string(), domain.clone());
        let y = IntVariable::new("y".to_string(), domain);

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}
