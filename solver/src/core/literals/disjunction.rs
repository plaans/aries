use crate::core::literals::Lits;
use crate::core::*;
use std::borrow::Borrow;
use std::cmp::Reverse;
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

    /// Returns true if the clause is in simplified normal form
    pub(crate) fn is_simplified(literals: &[Lit]) -> bool {
        literals.is_sorted_by_key(|l| Reverse(*l))
            && literals.windows(2).all(|k| !(k[1].entails(k[0])))
            && literals
                .iter()
                .all(|&l| !l.absurd() && (!l.tautological() || literals.len() == 1))
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
        if self.literals[0].tautological() {
            return true;
        }
        // The bollowing checks if there is an instance of (l || !l) which is trivially tautological
        // This check is currently not done when constructing the disjunction but would be fused in the main deduplication phase (but would requiring inline the `dedup_by` implementation)
        // the following check is more thorough t
        for i in 0..(self.literals.len() - 1) {
            let l1 = self.literals[i + 1];
            let l2 = self.literals[i];
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
    type IntoIter = <Lits as IntoIterator>::IntoIter;

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
impl FromIterator<Lit> for Disjunction {
    fn from_iter<T: IntoIterator<Item = Lit>>(iter: T) -> Self {
        DisjunctionBuilder::from_iter(iter).build()
    }
}

impl FromIterator<Lit> for DisjunctionBuilder {
    fn from_iter<T: IntoIterator<Item = Lit>>(iter: T) -> Self {
        let mut builder = DisjunctionBuilder::new();
        for l in iter {
            builder.push(l);
        }
        builder
    }
}

impl<const N: usize> From<[Lit; N]> for Disjunction {
    fn from(lits: [Lit; N]) -> Self {
        Disjunction::from_slice(lits.as_slice())
    }
}
impl From<&[Lit]> for Disjunction {
    fn from(value: &[Lit]) -> Self {
        Self::from_slice(value)
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

/// A builder for a disjunction. The benefit over a [`Lits`] vector, is that this one will
/// eagerly simplify when submitting tautological or absurd literals.
///
/// Other redundancies will only be eliminated when building the disjunction.
#[derive(Clone)]
pub struct DisjunctionBuilder {
    lits: Lits,
}

impl DisjunctionBuilder {
    pub fn new() -> Self {
        Self { lits: Lits::new() }
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            lits: Lits::with_capacity(n),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lits.is_empty()
    }

    pub fn tautological(&self) -> bool {
        self.lits.first().is_some_and(|l| l.tautological())
    }

    /// Adds an element to the disjunction, appropriately simplifying when submitting absurd or tautological literals.
    pub fn push(&mut self, lit: Lit) {
        if self.tautological() {
            // clause is always true no need to consider any new submitted literal
            return;
        }
        if lit.absurd() {
            return;
        }
        if lit.tautological() {
            // maintain the invariant that if we had any tautological literal, there is only one and it is the first
            self.lits.elems.resize(1, Lit::TRUE);
            self.lits[0] = Lit::TRUE;
        } else {
            self.lits.push(lit);
        }
    }

    /// Build the disjunction, reusing any allocation that the builder had.
    pub fn build(self) -> Disjunction {
        Disjunction::new(self.lits)
    }
}

impl Default for DisjunctionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<DisjunctionBuilder> for Disjunction {
    fn from(value: DisjunctionBuilder) -> Self {
        value.build()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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

    fn unordered(lits: &[Lit]) -> BTreeSet<Lit> {
        lits.iter().copied().collect()
    }

    #[test]
    fn test_clause_construction() {
        fn check(input: Vec<Lit>, output: Vec<Lit>) {
            let clause = Disjunction::from_vec(input);
            assert_eq!(unordered(&clause), unordered(&output));
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

        check(vec![Lit::FALSE, geq(A, 0), geq(A, 1)], vec![geq(A, 0)]);
        check(vec![Lit::TRUE, geq(A, 0), geq(A, 1)], vec![Lit::TRUE]);
        check(vec![Lit::FALSE, Lit::TRUE, geq(A, 0), geq(A, 1)], vec![Lit::TRUE]);
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
    fn test_minimality_coherence() {
        let vars = (0..=100).map(VarRef::from_u32);

        let vals = -5..5;

        // create a large set of literals from which to generate disjunction
        let mut lits = Vec::new();
        for var in vars {
            for val in vals.clone() {
                lits.push(Lit::geq(var, val));
                lits.push(Lit::leq(var, val));
            }
        }

        // select many subsets of the literals and test if going through the builder yields the correct output
        for _ in 0..10000 {
            lits.shuffle(&mut thread_rng());
            let subset = &lits[0..30];
            let disj = Disjunction::from_slice(subset);
            assert!(Disjunction::is_simplified(&disj));
        }
    }

    #[test]
    fn test_builder() {
        let vars = (1..=10).map(VarRef::from_u32);

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
