use crate::traits::Identifiable;
use crate::types::Id;


#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct VarBool {
    id: Id,
}

impl VarBool {
    pub(crate) fn new(id: Id) -> Self {
        VarBool { id }
    }
}

impl Identifiable for VarBool {
    fn id(&self) -> &Id {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        let x = VarBool::new("x".to_string());
        let y = VarBool::new("y".to_string());

        assert_eq!(x, x);
        assert_ne!(x, y);
        assert_eq!(y, y);
    }
}