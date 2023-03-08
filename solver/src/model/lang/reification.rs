use crate::core::*;
use crate::reif::ReifExpr;
use std::collections::HashMap;

/// A structure to keep track of all reification of expressions.
///
/// A correspondence between canonical expressions and the literal that they have reified to is maintained.
#[derive(Default, Clone)]
pub struct Reification {
    /// Associates each canonical atom to a single literal.
    map: HashMap<ReifExpr, Lit>,
}

impl Reification {
    /// If this expression was previously interned, returns the literal it was bound to.
    pub fn interned(&mut self, e: &ReifExpr) -> Option<Lit> {
        match e {
            ReifExpr::Lit(l) => Some(*l),
            _ => self.map.get(e).copied(),
        }
    }

    /// Interns the user-facing expression.
    /// Panics, if the expression is already interned.
    pub fn intern_as(&mut self, e: ReifExpr, lit: Lit) {
        assert!(!self.map.contains_key(&e));
        self.map.insert(e.clone(), lit);
        self.map.insert(!e, !lit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lang::expr::{geq, leq};
    use crate::model::lang::IVar;

    const A: IVar = IVar::new(VarRef::from_u32(1));
    const B: IVar = IVar::new(VarRef::from_u32(2));
    const C: IVar = IVar::new(VarRef::from_u32(3));

    #[test]
    fn test_reif() {
        let t = Lit::TRUE;
        let f = Lit::FALSE;
        let l1: ReifExpr = leq(A, B + 3).into();
        let l2: ReifExpr = leq(A, C).into();

        let mut reif = Reification::default();
        reif.intern_as(l1.clone(), t);
        reif.intern_as(l2.clone(), f);

        assert_eq!(reif.interned(&l1), Some(t));
        assert_eq!(reif.interned(&l2), Some(f));

        // same as l1
        let l1_prime = geq(B + 3, A).into();
        assert_eq!(reif.interned(&l1_prime), Some(t));

        // inverse of l1, should return the opposite literal
        assert_eq!(reif.interned(&(!l1.clone())), Some(f));
    }
}
