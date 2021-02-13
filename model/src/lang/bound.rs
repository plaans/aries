use crate::int_model::{DomEvent, VarEvent};
use crate::lang::boolean::BVar;
use crate::lang::{IntCst, VarRef};
use core::convert::{From, Into};
use std::borrow::Borrow;
use std::cmp::Ordering;

/// A `Bound` represents a a lower or upper bound on a discrete variable
/// (i.e. an integer, boolean or symbolic variable).
///
/// For a boolean variable X:
///  - the bound `x > 0` represent the true literal (`X` takes the value `true`)
///  - the bound `x <= 0` represents the false literal (`X` takes the value `false`)
///
/// ```
/// use aries_model::Model;
/// use aries_model::lang::Bound;
/// let mut model = Model::new();
/// let x = model.new_bvar("X");
/// let x_is_true: Bound = x.true_lit();
/// let x_is_false: Bound = x.false_lit();
/// let y = model.new_ivar(0, 10, "Y");
/// let y_geq_5 = Bound::geq(y, 5);
/// ```
/// TODO: look into bitfields to bring this down to 64 bits
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum Bound {
    LEQ(VarRef, IntCst),
    GT(VarRef, IntCst),
}
impl PartialOrd for Bound {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Bound {
    fn cmp(&self, other: &Self) -> Ordering {
        self.lexical_cmp(other)
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum Relation {
    LEQ,
    GT,
}

impl Bound {
    pub fn variable(&self) -> VarRef {
        match self {
            Bound::LEQ(v, _) => *v,
            Bound::GT(v, _) => *v,
        }
    }

    pub fn leq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::LEQ(var.into(), val)
    }
    pub fn lt(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::leq(var, val - 1)
    }
    pub fn geq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::gt(var, val - 1)
    }
    pub fn gt(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::GT(var.into(), val)
    }

    pub fn is_true(v: BVar) -> Bound {
        Bound::geq(v, 1)
    }
    pub fn is_false(v: BVar) -> Bound {
        Bound::leq(v, 0)
    }

    pub fn made_true_by(&self, event: &VarEvent) -> bool {
        let neg = !*self;
        neg.made_false_by(event)
    }
    pub fn made_false_by(&self, event: &VarEvent) -> bool {
        if self.var() != event.var {
            return false;
        }
        match self {
            Bound::LEQ(_, upper_bound) => {
                if let DomEvent::NewLB { prev, new } = event.ev {
                    prev <= *upper_bound && *upper_bound < new
                } else {
                    false
                }
            }
            Bound::GT(_, val) => {
                let lower_bound = val + 1;
                if let DomEvent::NewUB { prev, new } = event.ev {
                    lower_bound > new && prev >= lower_bound
                } else {
                    false
                }
            }
        }
    }

    pub fn entails(&self, other: Bound) -> bool {
        if self.var() != other.var() {
            return false;
        }
        match self {
            Bound::LEQ(_, upper_bound) => {
                if let Bound::LEQ(_, o) = other {
                    o >= *upper_bound
                } else {
                    false
                }
            }
            Bound::GT(_, val) => {
                if let Bound::GT(_, o) = other {
                    o <= *val
                } else {
                    false
                }
            }
        }
    }

    pub fn var(&self) -> VarRef {
        match self {
            Bound::LEQ(v, _) => *v,
            Bound::GT(v, _) => *v,
        }
    }

    fn as_triple(&self) -> (VarRef, Relation, IntCst) {
        match self {
            Bound::LEQ(var, val) => (*var, Relation::LEQ, *val),
            Bound::GT(var, val) => (*var, Relation::GT, *val),
        }
    }

    /// An ordering that will group bounds by (given from highest to lowest priority):
    ///  - variable
    ///  - affected bound (lower, upper)
    ///  - by value of the bound
    pub fn lexical_cmp(&self, other: &Bound) -> Ordering {
        self.as_triple().cmp(&other.as_triple())
    }
}

impl std::ops::Not for Bound {
    type Output = Bound;

    fn not(self) -> Self::Output {
        match self {
            Bound::LEQ(var, val) => Bound::GT(var, val),
            Bound::GT(var, val) => Bound::LEQ(var, val),
        }
    }
}

impl From<BVar> for Bound {
    fn from(v: BVar) -> Self {
        v.true_lit()
    }
}

impl From<VarEvent> for Bound {
    fn from(ev: VarEvent) -> Self {
        match ev.ev {
            DomEvent::NewLB { new: new_lb, .. } => Bound::geq(ev.var, new_lb),
            DomEvent::NewUB { new: new_ub, .. } => Bound::leq(ev.var, new_ub),
        }
    }
}

impl std::fmt::Debug for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bound::LEQ(var, val) => write!(f, "{:?} <= {}", var, val),
            Bound::GT(var, val) => write!(f, "{:?} > {}", var, val),
        }
    }
}

/// A set of literals representing a disjunction.
/// A `Disjunction` maintains the invariant that there are not duplicated literals (a literal that entails another one).
/// Implementation maintains the literals sorted.
pub struct Disjunction {
    literals: Vec<Bound>,
}

impl Disjunction {
    pub fn new(mut literals: Vec<Bound>) -> Self {
        if literals.len() <= 1 {
            return Disjunction { literals };
        }
        // sort literals, so that they are grouped by (1) variable and (2) affected bound
        literals.sort();
        // remove duplicated literals
        let mut i = 0;
        while i < literals.len() - 1 {
            if literals[i].entails(literals[i + 1]) {
                literals.remove(i);
            } else if literals[i + 1].entails(literals[i]) {
                literals.remove(i + 1);
            } else {
                i += 1;
            }
        }

        Disjunction { literals }
    }

    pub fn new_non_tautological(literals: Vec<Bound>) -> Option<Disjunction> {
        let disj = Disjunction::new(literals);
        if disj.is_tautology() {
            None
        } else {
            Some(disj)
        }
    }

    /// Returns true if the clause is always true
    pub fn is_tautology(&self) -> bool {
        for i in 0..(self.literals.len() - 1) {
            let l1 = self.literals[i];
            let l2 = self.literals[i + 1];
            debug_assert!(l1 < l2, "clause is not sorted");
            if l1.var() == l2.var() {
                match (l1, l2) {
                    // due to the invariants of the clause, we are always in this case
                    (Bound::LEQ(_, x), Bound::GT(_, y)) => {
                        // we have the disjunction var <= x || var > y
                        // if y <= x, all values of var satisfy one of the disjuncts
                        if y <= x {
                            return true;
                        }
                    }
                    _ => panic!("Invariant violated on the clause"),
                }
            }
        }
        false
    }

    pub fn literals(&self) -> &[Bound] {
        &self.literals
    }
}

impl From<Vec<Bound>> for Disjunction {
    fn from(literals: Vec<Bound>) -> Self {
        Disjunction::new(literals)
    }
}
impl From<Disjunction> for Vec<Bound> {
    fn from(cl: Disjunction) -> Self {
        cl.literals
    }
}

impl Borrow<[Bound]> for Disjunction {
    fn borrow(&self) -> &[Bound] {
        &self.literals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lit_implication() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);

        // event for a domain change of [0, X] to [9, X]
        let ea_lb = VarEvent {
            var: a,
            ev: DomEvent::NewLB { prev: 0, new: 9 },
        };
        // event for a domain change of [X, 10] to [X, 1]
        let ea_ub = VarEvent {
            var: a,
            ev: DomEvent::NewUB { prev: 10, new: 1 },
        };

        // ===== lower bounds ======

        let lit = Bound::LEQ(a, 5);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        let lit = Bound::LEQ(a, 0);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = Bound::LEQ(a, -1);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // ===== upper bounds =====

        let lit = Bound::geq(a, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(lit.made_false_by(&ea_ub));

        let lit = Bound::geq(a, 10);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = Bound::geq(a, 11);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // ===== unrelated variable =====

        // events on b, should not match
        let lit = Bound::LEQ(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
        let lit = Bound::GT(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
    }

    fn leq(var: VarRef, val: IntCst) -> Bound {
        Bound::leq(var, val)
    }
    fn geq(var: VarRef, val: IntCst) -> Bound {
        Bound::geq(var, val)
    }

    #[test]
    fn test_entailments() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);

        assert!(leq(a, 0).entails(leq(a, 0)));
        assert!(leq(a, 0).entails(leq(a, 1)));
        assert!(!leq(a, 0).entails(leq(a, -1)));

        assert!(!leq(a, 0).entails(leq(b, 0)));
        assert!(!leq(a, 0).entails(leq(b, 1)));
        assert!(!leq(a, 0).entails(leq(b, -1)));

        assert!(geq(a, 0).entails(geq(a, 0)));
        assert!(!geq(a, 0).entails(geq(a, 1)));
        assert!(geq(a, 0).entails(geq(a, -1)));

        assert!(!geq(a, 0).entails(geq(b, 0)));
        assert!(!geq(a, 0).entails(geq(b, 1)));
        assert!(!geq(a, 0).entails(geq(b, -1)));
    }

    #[test]
    fn test_clause_construction() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);

        fn check(input: Vec<Bound>, output: Vec<Bound>) {
            let clause = Disjunction::new(input);
            let simplified = Vec::from(clause);
            assert_eq!(simplified, output);
        };
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
