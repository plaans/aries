use crate::core::literals::Lits;
use crate::core::*;
use std::borrow::Borrow;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;

/// A set of literals representing a disjunction in a normalized form.
/// A `Disjunction` maintains the invariant that there are not duplicated literals (a literal that entails another one) and that its elements are ordered.
#[derive(PartialEq, Clone, Eq, Hash)]
pub struct Conjunction {
    literals: Lits,
}
impl Debug for Conjunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.literals)
    }
}

impl Conjunction {
    pub fn tautology() -> Self {
        Self { literals: Lits::new() }
    }

    pub fn new(mut literals: Lits) -> Self {
        literals.simplify_conjunctive();
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
        literals.is_sorted()
            && literals.windows(2).all(|k| !(k[0].entails(k[1])))
            && literals
                .iter()
                .all(|&l| !l.tautological() && (!l.absurd() || literals.len() == 1))
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
impl<'a> IntoIterator for &'a Conjunction {
    type Item = Lit;
    type IntoIter = <&'a Lits as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.as_ref().iter().copied()
    }
}
impl IntoIterator for Conjunction {
    type Item = Lit;
    type IntoIter = <Lits as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.into_iter()
    }
}

impl From<Lits> for Conjunction {
    fn from(value: Lits) -> Self {
        Self::new(value)
    }
}
impl From<&Lits> for Conjunction {
    fn from(value: &Lits) -> Self {
        Self::from_slice(value.as_slice())
    }
}
impl From<Vec<Lit>> for Conjunction {
    fn from(literals: Vec<Lit>) -> Self {
        Self::from_vec(literals)
    }
}
impl<'a> From<&'a Vec<Lit>> for Conjunction {
    fn from(literals: &'a Vec<Lit>) -> Self {
        Self::from_slice(literals.as_slice())
    }
}

impl<const N: usize> From<[Lit; N]> for Conjunction {
    fn from(lits: [Lit; N]) -> Self {
        Conjunction::from_slice(lits.as_slice())
    }
}
impl From<&[Lit]> for Conjunction {
    fn from(value: &[Lit]) -> Self {
        Self::from_slice(value)
    }
}

impl From<&Conjunction> for Conjunction {
    fn from(dis: &Conjunction) -> Self {
        dis.clone()
    }
}

impl FromIterator<Lit> for Conjunction {
    fn from_iter<T: IntoIterator<Item = Lit>>(iter: T) -> Self {
        ConjunctionBuilder::from_iter(iter).build()
    }
}

impl FromIterator<Lit> for ConjunctionBuilder {
    fn from_iter<T: IntoIterator<Item = Lit>>(iter: T) -> Self {
        let mut builder = ConjunctionBuilder::new();
        for l in iter {
            builder.push(l);
        }
        builder
    }
}

impl Borrow<[Lit]> for Conjunction {
    fn borrow(&self) -> &[Lit] {
        &self.literals
    }
}

impl AsRef<[Lit]> for Conjunction {
    fn as_ref(&self) -> &[Lit] {
        &self.literals
    }
}
impl Deref for Conjunction {
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
pub struct ConjunctionBuilder {
    lits: Lits,
}

impl ConjunctionBuilder {
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
        self.lits.is_empty()
    }

    pub fn absurd(&self) -> bool {
        self.lits.first().is_some_and(|l| l.absurd())
    }

    /// Adds an element to the disjunction, appropriately simplifying when submitting absurd or tautological literals.
    pub fn push(&mut self, lit: Lit) {
        if self.absurd() {
            // always false, no matter what we will add to it
            return;
        }
        if lit.tautological() {
            return;
        }
        if lit.absurd() {
            // maintain the invariant that if we had any absurd literal, there is only one and it is the first
            self.lits.elems.resize(1, Lit::FALSE);
            self.lits[0] = Lit::FALSE;
            debug_assert!(self.absurd())
        } else {
            self.lits.push(lit);
        }
    }

    /// Build the disjunction, reusing any allocation that the builder had.
    pub fn build(self) -> Conjunction {
        Conjunction::new(self.lits)
    }
}

impl Default for ConjunctionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ConjunctionBuilder> for Conjunction {
    fn from(value: ConjunctionBuilder) -> Self {
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
    fn test_conjunction_construction() {
        fn check(input: Vec<Lit>, output: Vec<Lit>) {
            let clause = Conjunction::from_vec(input);
            assert_eq!(unordered(&clause), unordered(&output));
        }
        check(vec![geq(A, 0), geq(A, 1)], vec![geq(A, 1)]);

        check(vec![leq(A, 0), leq(A, 1)], vec![leq(A, 0)]);
        check(vec![leq(A, 1), leq(A, 0)], vec![leq(A, 0)]);
        check(vec![leq(A, 0), leq(A, 0)], vec![leq(A, 0)]);
        check(vec![leq(A, 0), leq(A, 1), leq(A, 1), leq(A, 0)], vec![leq(A, 0)]);

        check(vec![leq(A, 0), !leq(A, 0)], vec![leq(A, 0), !leq(A, 0)]);

        check(
            vec![leq(A, 0), leq(B, 1), leq(A, 1), leq(B, 0)],
            vec![leq(A, 0), leq(B, 0)],
        );
        check(
            vec![geq(A, 0), geq(B, 1), geq(A, 1), geq(B, 0)],
            vec![geq(A, 1), geq(B, 1)],
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
            vec![leq(A, 0), geq(A, 1), leq(B, 0), geq(B, 1)],
        );

        check(vec![Lit::FALSE, geq(A, 0), geq(A, 1)], vec![Lit::FALSE]);
        check(vec![Lit::TRUE, geq(A, 0), geq(A, 1)], vec![geq(A, 1)]);
        check(vec![Lit::FALSE, Lit::TRUE, geq(A, 0), geq(A, 1)], vec![Lit::FALSE]);
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
            let disj = Conjunction::from_slice(subset);
            assert!(Conjunction::is_simplified(&disj));
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
            let disj = Conjunction::from_slice(subset);
            let mut builder = ConjunctionBuilder::new();
            for l in subset {
                builder.push(*l);
            }
            let built: Conjunction = builder.into();
            assert_eq!(disj, built);
        }
    }
}
