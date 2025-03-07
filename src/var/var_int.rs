use crate::domain::IntDomain;
use crate::traits::Identifiable;
use crate::types::Id;

#[derive(Clone, Eq, Hash, Debug)]
pub struct VarInt {
    id: Id,
    domain: IntDomain,
}

impl VarInt {
    /// Create a new `IntVariable` with the given id and domain.
    pub(crate) fn new(id: Id, domain: IntDomain) -> Self {
        VarInt { id, domain }
    }

    /// Return the variable domain.
    pub fn domain(&self) -> &IntDomain {
        &self.domain
    }
}

impl Identifiable for VarInt {
    fn id(&self) -> &Id {
        &self.id
    }
}

impl PartialEq for VarInt {
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

        let x = VarInt::new("x".to_string(), domain.clone());
        let y = VarInt::new("y".to_string(), domain);

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}
