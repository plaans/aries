use crate::core::literals::Lits;
use crate::core::*;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;

/// A set of literals representing a disjunction in a normalized form.
/// A `Disjunction` maintains the invariant that there are not duplicated literals (a literal that entails another one) and that its elements are ordered.
#[derive(PartialEq, Clone, Eq, Hash)]
pub struct Disjunction {
    literals: Lits,
}
impl Debug for Disjunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.literals)
    }
}

impl Disjunction {
    pub fn contradiction() -> Self {
        Self { literals: Lits::new() }
    }

    pub fn new(mut literals: Lits) -> Self {
        literals.simplify_disjunctive();
        Self { literals }
    }

    pub fn from_vec(literals: Vec<Lit>) -> Self {
        Self::new(Lits::from_vec(literals))
    }

    pub fn from_slice(literals: impl Borrow<[Lit]>) -> Self {
        Self::new(Lits::from_slice(literals))
    }

    pub fn into_lits(self) -> Lits {
        self.literals
    }

    /// Returns true if the clause is simplified (no redundant literals.)
    pub(crate) fn is_simplified(literals: &[Lit]) -> bool {
        literals.is_sorted() && literals.windows(2).all(|k| !(k[0].entails(k[1])))
    }

    pub fn new_non_tautological(literals: Lits) -> Option<Disjunction> {
        let disj = Disjunction::new(literals);
        if disj.is_tautology() {
            None
        } else {
            Some(disj)
        }
    }

    /// Returns true if the clause is always true
    pub fn is_tautology(&self) -> bool {
        if self.is_empty() {
            return false;
        }
        for i in 0..(self.literals.len() - 1) {
            let l1 = self.literals[i];
            let l2 = self.literals[i + 1];
            debug_assert!(l1 < l2, "clause is not sorted");
            if l1.variable() == l2.variable() {
                debug_assert!(l1.svar().is_minus());
                debug_assert!(l2.svar().is_plus());
                if (!l1).entails(l2) || (!l2).entails(l1) {
                    // all values of var satisfy one of the disjuncts
                    return true;
                }
            }
        }
        false
    }

    pub fn retain<F: FnMut(Lit) -> bool>(&mut self, f: F) {
        self.literals.retain(f);
    }

    pub fn literals(&self) -> &[Lit] {
        &self.literals
    }
    pub fn iter(&self) -> impl Iterator<Item = Lit> + '_ {
        self.literals.iter()
    }

    pub fn len(&self) -> usize {
        self.literals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    pub fn contains(&self, lit: Lit) -> bool {
        self.literals.contains(&lit)
    }
}
impl<'a> IntoIterator for &'a Disjunction {
    type Item = Lit;
    type IntoIter = <&'a Lits as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.as_ref().iter().copied()
    }
}
impl IntoIterator for Disjunction {
    type Item = Lit;
    type IntoIter = <Lits as IntoIterator>::IntoIter; //    std::iter::Copied<std::slice::Iter<'a, Lit>>;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.into_iter()
    }
}

impl From<Lits> for Disjunction {
    fn from(value: Lits) -> Self {
        Self::new(value)
    }
}
impl From<&Lits> for Disjunction {
    fn from(value: &Lits) -> Self {
        Self::from_slice(value.as_slice())
    }
}
impl From<Vec<Lit>> for Disjunction {
    fn from(literals: Vec<Lit>) -> Self {
        Self::from_vec(literals)
    }
}
impl<'a> From<&'a Vec<Lit>> for Disjunction {
    fn from(literals: &'a Vec<Lit>) -> Self {
        Self::from_slice(literals.as_slice())
    }
}

impl<const N: usize> From<[Lit; N]> for Disjunction {
    fn from(lits: [Lit; N]) -> Self {
        Disjunction::from_slice(lits.as_slice())
    }
}

impl From<&Disjunction> for Disjunction {
    fn from(dis: &Disjunction) -> Self {
        dis.clone()
    }
}

impl Borrow<[Lit]> for Disjunction {
    fn borrow(&self) -> &[Lit] {
        &self.literals
    }
}

impl AsRef<[Lit]> for Disjunction {
    fn as_ref(&self) -> &[Lit] {
        &self.literals
    }
}
impl Deref for Disjunction {
    type Target = [Lit];

    fn deref(&self) -> &Self::Target {
        &self.literals
    }
}

/// A builder for a disjunction that avoids duplicated literals
#[derive(Default, Clone)]
pub struct DisjunctionBuilder {
    upper_bounds: HashMap<SignedVar, IntCst>,
}

impl DisjunctionBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            upper_bounds: HashMap::with_capacity(n),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.upper_bounds.is_empty()
    }

    pub fn push(&mut self, lit: Lit) {
        let sv = lit.svar();
        let ub = lit.ub_value();
        let new_ub = if let Some(prev) = self.upper_bounds.get(&sv) {
            // (sv <= ub) || (sv <= prev)  <=> (sv <= max(ub, prev))
            ub.max(*prev)
        } else {
            ub
        };
        self.upper_bounds.insert(sv, new_ub);
    }

    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
        self.upper_bounds.iter().map(|(k, v)| Lit::leq(*k, *v))
    }
}

impl From<DisjunctionBuilder> for Disjunction {
    fn from(value: DisjunctionBuilder) -> Self {
        Self::new(Lits::from_iter(value.literals()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    const A: VarRef = VarRef::from_u32(1u32);
    const B: VarRef = VarRef::from_u32(2u32);

    fn leq(var: VarRef, val: IntCst) -> Lit {
        Lit::leq(var, val)
    }
    fn geq(var: VarRef, val: IntCst) -> Lit {
        Lit::geq(var, val)
    }

    #[test]
    fn test_clause_construction() {
        fn check(input: Vec<Lit>, mut output: Vec<Lit>) {
            let clause = Disjunction::from_vec(input);
            output.sort_unstable();
            assert_eq!(clause.literals(), output.as_slice());
        }
        // (a >= 0) || (a >= 1)   <=>   (a >= 0)
        check(vec![geq(A, 0), geq(A, 1)], vec![geq(A, 0)]);

        // (a <= 0) || (a <= 1)   <=>   (a <= 1)
        check(vec![leq(A, 0), leq(A, 1)], vec![leq(A, 1)]);
        check(vec![leq(A, 1), leq(A, 0)], vec![leq(A, 1)]);
        check(vec![leq(A, 0), leq(A, 0)], vec![leq(A, 0)]);
        check(vec![leq(A, 0), leq(A, 1), leq(A, 1), leq(A, 0)], vec![leq(A, 1)]);

        check(vec![leq(A, 0), !leq(A, 0)], vec![leq(A, 0), !leq(A, 0)]);

        check(
            vec![leq(A, 0), leq(B, 1), leq(A, 1), leq(B, 0)],
            vec![leq(A, 1), leq(B, 1)],
        );
        check(
            vec![geq(A, 0), geq(B, 1), geq(A, 1), geq(B, 0)],
            vec![geq(A, 0), geq(B, 0)],
        );
        check(
            vec![
                leq(A, 0),
                leq(B, 1),
                leq(A, 1),
                leq(B, 0),
                geq(A, 0),
                geq(B, 1),
                geq(A, 1),
                geq(B, 0),
            ],
            vec![leq(A, 1), geq(A, 0), leq(B, 1), geq(B, 0)],
        );
    }

    #[test]
    fn test_tautology() {
        assert!(Disjunction::from_vec(vec![leq(A, 0), !leq(A, 0)]).is_tautology());
        // a <= 0 || a > -1
        //           a <= -1
        assert!(Disjunction::from([leq(A, 0), geq(A, 0)]).is_tautology());
        assert!(Disjunction::from([leq(A, 0), geq(A, 1)]).is_tautology());
        assert!(Disjunction::from([leq(A, 0), leq(B, 0), geq(B, 2), !leq(A, 0)]).is_tautology());
    }

    #[test]
    fn test_builder() {
        let vars = (0..10).map(VarRef::from_u32);

        let vals = 0..10;

        // create a large set of literals from which to generate disjunction
        let mut lits = Vec::new();
        for var in vars {
            for val in vals.clone() {
                lits.push(Lit::geq(var, val));
                lits.push(Lit::leq(var, val));
            }
        }

        // select many subsets of the literals and test if going through the builder yields the correct output
        for _ in 0..100 {
            lits.shuffle(&mut thread_rng());
            let subset = &lits[0..30];
            let disj = Disjunction::from_slice(subset);
            let mut builder = DisjunctionBuilder::new();
            for l in subset {
                builder.push(*l);
            }
            let built: Disjunction = builder.into();
            assert_eq!(disj, built);
        }
    }
}
