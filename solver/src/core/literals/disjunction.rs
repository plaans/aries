use crate::core::*;
use std::borrow::Borrow;
use std::fmt::{Debug, Formatter};

/// A set of literals representing a disjunction.
/// A `Disjunction` maintains the invariant that there are not duplicated literals (a literal that entails another one).
/// Implementation maintains the literals sorted.
#[derive(PartialEq, Clone, Eq, Hash)]
pub struct Disjunction {
    literals: Vec<Lit>,
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
                debug_assert_eq!(l1.relation(), Relation::Gt);
                debug_assert_eq!(l2.relation(), Relation::Leq);
                let x = l1.value();
                let y = l2.value();
                // we have the disjunction var > x || var <= y
                // if y > x, all values of var satisfy one of the disjuncts
                if y >= x {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
