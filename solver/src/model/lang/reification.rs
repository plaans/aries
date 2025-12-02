use crate::core::*;
use crate::reif::ReifExpr;
use hashbrown::HashMap;

/// A structure to keep track of all reification of expressions.
///
/// A correspondence between canonical expressions and the literal that they have reified to is maintained.
#[derive(Default, Clone)]
pub struct Reification {
    /// Associates each canonical atom to a single literal.
    full_map: HashMap<ReifExpr, Lit>,
    full_inv: HashMap<Lit, ReifExpr>,
    half_map: HashMap<ReifExpr, Lit>,
    half_inv: HashMap<Lit, ReifExpr>,
}

impl Reification {
    /// If this expression was previously interned, returns the literal it was bound to.
    pub fn interned_full(&mut self, e: &ReifExpr) -> Option<Lit> {
        match e {
            ReifExpr::Lit(l) => Some(*l),
            _ => self.full_map.get(e).copied(),
        }
    }
    pub fn interned_half(&mut self, e: &ReifExpr) -> Option<Lit> {
        match e {
            ReifExpr::Lit(l) => Some(*l),
            _ => self.half_map.get(e).copied(),
        }
    }

    /// Interns the user-facing expression.
    /// Panics, if the expression is already interned.
    pub fn intern_full_as(&mut self, e: ReifExpr, lit: Lit) {
        assert!(!self.full_map.contains_key(&e));
        self.full_map.insert(e.clone(), lit);
        self.full_inv.insert(lit, e.clone());
        debug_assert!(e.negatable(), "Full reification of non-negatable expression");
        self.full_map.insert(!e.clone(), !lit);
        self.full_inv.insert(!lit, !e.clone());
    }

    pub fn intern_half_as(&mut self, e: ReifExpr, lit: Lit) {
        assert!(!self.half_map.contains_key(&e));
        self.half_map.insert(e.clone(), lit);
        self.half_inv.insert(lit, e.clone());
    }

    pub fn original_full(&self, lit: Lit) -> Option<&ReifExpr> {
        self.full_inv.get(&lit)
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
        reif.intern_full_as(l1.clone(), t);
        reif.intern_full_as(l2.clone(), f);

        assert_eq!(reif.interned_full(&l1), Some(t));
        assert_eq!(reif.interned_full(&l2), Some(f));

        // same as l1
        let l1_prime = geq(B + 3, A).into();
        assert_eq!(reif.interned_full(&l1_prime), Some(t));

        // inverse of l1, should return the opposite literal
        assert_eq!(reif.interned_full(&(!l1.clone())), Some(f));
    }
}
