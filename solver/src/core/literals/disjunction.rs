use crate::core::*;
use itertools::Itertools;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

/// A set of literals representing a disjunction.
/// A `Disjunction` maintains the invariant that there are not duplicated literals (a literal that entails another one).
/// Implementation maintains the literals sorted.
#[derive(PartialEq, Clone, Eq, Hash)]
pub struct Disjunction {
    pub(crate) literals: Vec<Lit>,
}
impl Debug for Disjunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.literals)
    }
}

impl Disjunction {
    pub fn new(mut literals: Vec<Lit>) -> Self {
        if literals.len() <= 1 {
            return Disjunction { literals };
        }
        // sort literals, so that they are grouped by (1) variable and (2) affected bound
        literals.sort();
        // remove duplicated literals
        let mut i = 0;
        while i < literals.len() - 1 {
            // because of the ordering properties, we can only check entailment for the immediately following element
            if literals[i].entails(literals[i + 1]) {
                literals.remove(i);
            } else {
                i += 1;
            }
        }

        Disjunction { literals }
    }

    pub fn new_non_tautological(literals: Vec<Lit>) -> Option<Disjunction> {
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

    pub fn literals(&self) -> &[Lit] {
        &self.literals
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
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, Lit>>;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.iter().copied()
    }
}
impl IntoIterator for Disjunction {
    type Item = Lit;
    type IntoIter = std::vec::IntoIter<Lit>;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.into_iter()
    }
}

impl From<Vec<Lit>> for Disjunction {
    fn from(literals: Vec<Lit>) -> Self {
        Disjunction::new(literals)
    }
}
impl<'a> From<&'a Vec<Lit>> for Disjunction {
    fn from(literals: &'a Vec<Lit>) -> Self {
        Disjunction::new(literals.clone())
    }
}
impl From<Disjunction> for Vec<Lit> {
    fn from(cl: Disjunction) -> Self {
        cl.literals
    }
}

impl<const N: usize> From<[Lit; N]> for Disjunction {
    fn from(lits: [Lit; N]) -> Self {
        Disjunction::new(lits.into())
    }
}

impl From<&Disjunction> for Disjunction {
    fn from(dis: &Disjunction) -> Self {
        Disjunction {
            literals: dis.literals.clone(),
        }
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
        Disjunction::new(value.literals().collect_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    fn leq(var: VarRef, val: IntCst) -> Lit {
        Lit::leq(var, val)
    }
    fn geq(var: VarRef, val: IntCst) -> Lit {
        Lit::geq(var, val)
    }

    #[test]
    fn test_clause_construction() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);

        fn check(input: Vec<Lit>, mut output: Vec<Lit>) {
            let clause = Disjunction::new(input);
            let simplified = Vec::from(clause);
            output.sort_unstable();
            assert_eq!(simplified, output);
        }
        // (a >= 0) || (a >= 1)   <=>   (a >= 0)
        check(vec![geq(a, 0), geq(a, 1)], vec![geq(a, 0)]);

        // (a <= 0) || (a <= 1)   <=>   (a <= 1)
        check(vec![leq(a, 0), leq(a, 1)], vec![leq(a, 1)]);
        check(vec![leq(a, 1), leq(a, 0)], vec![leq(a, 1)]);
        check(vec![leq(a, 0), leq(a, 0)], vec![leq(a, 0)]);
        check(vec![leq(a, 0), leq(a, 1), leq(a, 1), leq(a, 0)], vec![leq(a, 1)]);

        check(vec![leq(a, 0), !leq(a, 0)], vec![leq(a, 0), !leq(a, 0)]);

        check(
            vec![leq(a, 0), leq(b, 1), leq(a, 1), leq(b, 0)],
            vec![leq(a, 1), leq(b, 1)],
        );
        check(
            vec![geq(a, 0), geq(b, 1), geq(a, 1), geq(b, 0)],
            vec![geq(a, 0), geq(b, 0)],
        );
        check(
            vec![
                leq(a, 0),
                leq(b, 1),
                leq(a, 1),
                leq(b, 0),
                geq(a, 0),
                geq(b, 1),
                geq(a, 1),
                geq(b, 0),
            ],
            vec![leq(a, 1), geq(a, 0), leq(b, 1), geq(b, 0)],
        );
    }

    #[test]
    fn test_tautology() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);

        assert!(Disjunction::new(vec![leq(a, 0), !leq(a, 0)]).is_tautology());
        // a <= 0 || a > -1
        //           a <= -1
        assert!(Disjunction::new(vec![leq(a, 0), geq(a, 0)]).is_tautology());
        assert!(Disjunction::new(vec![leq(a, 0), geq(a, 1)]).is_tautology());
        assert!(Disjunction::new(vec![leq(a, 0), leq(b, 0), geq(b, 2), !leq(a, 0)]).is_tautology());
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
            let disj = Disjunction::new(subset.to_vec());
            let mut builder = DisjunctionBuilder::new();
            for l in subset {
                builder.push(*l);
            }
            let built: Disjunction = builder.into();
            assert_eq!(disj, built);
        }
    }
}
