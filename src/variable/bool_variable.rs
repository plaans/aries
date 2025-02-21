use crate::traits::Identifiable;
use crate::types::Id;


#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BoolVariable {
    id: Id,
}

impl BoolVariable {
    /// Create a new `BoolVariable` with the given id.
    pub fn new(id: Id) -> Self {
        BoolVariable { id }
    }
}

impl Identifiable for BoolVariable {
    fn id(&self) -> &Id {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        let x = BoolVariable::new("x".to_string());
        let y = BoolVariable::new("y".to_string());

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}