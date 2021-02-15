use crate::bounds::var_bound::VarBound;
use crate::bounds::BoundValue;
use crate::int_model::{DomEvent, VarEvent};
use crate::lang::BVar;
use crate::lang::{IntCst, VarRef};
use core::convert::{From, Into};
use std::cmp::Ordering;
use std::mem::transmute;

/// A `Bound` represents a lower or upper bound on a discrete variable
/// (i.e. an integer, boolean or symbolic variable).
///
/// For a boolean variable X:
///  - the bound `x > 0` represent the true literal (`X` takes the value `true`)
///  - the bound `x <= 0` represents the false literal (`X` takes the value `false`)
///
/// The struct is opaque as it is internal representation is optimized to allow more efficient usage.
/// To access indivual fields the methods `variable()`, `relation()` and `value()` can be used.
/// The `unpack()` method extract all fields into a tuple.
///
/// ```
/// use aries_model::Model;
/// use aries_model::lang::VarRef;
/// use aries_model::bounds::{Bound, Relation};
/// let mut model = Model::new();
/// let x = model.new_bvar("X");
/// let x_is_true: Bound = x.true_lit();
/// let x_is_false: Bound = x.false_lit();
/// let y = model.new_ivar(0, 10, "Y");
/// let y_geq_5 = Bound::geq(y, 5);
///
/// // the `<=` is internally converted into a `<`
/// // the variable is converted into a `VarRef`
/// let y: VarRef = y.into();
/// assert_eq!(y_geq_5.variable(), y);
/// assert_eq!(y_geq_5.relation(), Relation::GT);
/// assert_eq!(y_geq_5.value(), 4);
/// assert_eq!(y_geq_5.unpack(), (y, Relation::GT, 4));
/// ```
///
/// # Ordering
///
/// Bound define a very specific order, which is equivalent to sorting the result of the `unpack()` method.
/// The different fields are compared in the following order to define the ordering:
///  - variable
///  - relation
///  - value
///
/// As result, ordering a vector of bounds will group bounds by variable, then among bound on the same variable by relation.
/// An important invariant is that, in a sorted list, a bound can only entail the bounds immediatly following it.
///
/// ```
/// use aries_model::Model;
/// use aries_model::bounds::Bound;
/// let mut model = Model::new();
/// let x = model.new_ivar(0, 10, "X");
/// let y = model.new_ivar(0, 10, "Y");
/// let mut bounds = vec![Bound::leq(y, 4), Bound::geq(x,1), Bound::leq(x, 3), Bound::leq(x, 4), Bound::leq(x, 6), Bound::geq(x,2)];
/// bounds.sort();
/// assert_eq!(bounds, vec![Bound::leq(x, 3), Bound::leq(x, 4), Bound::leq(x, 6), Bound::geq(x,2), Bound::geq(x,1), Bound::leq(y, 4)]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Bound {
    /// Union of the variable (highest 31 bits) and the relation (lowest bit)
    /// This encoding allows:
    ///  - to very efficiently check whether two bounds have the same `(variable, relation)` part
    ///    which is one of the critical operation in `entails`.
    ///  - to use as an index in a table: each variable will have two slots: one of the LEQ relation
    ///    and one for the GT relation
    pub(in crate::bounds) var_rel: u32,
    /// +/- the value of the relation. The value of a GT relation is negated before being stored.
    /// This design allows to test entailment without testing the relation of the Bound
    pub(in crate::bounds) raw_value: BoundValue,
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum Relation {
    LEQ = 0,
    GT = 1,
}

impl std::ops::Not for Relation {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Relation::LEQ => Relation::GT,
            Relation::GT => Relation::LEQ,
        }
    }
}

impl std::fmt::Display for Relation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Relation::LEQ => write!(f, "<="),
            Relation::GT => write!(f, ">"),
        }
    }
}

const REL_MASK: u32 = 0x1;
const VAR_MASK: u32 = !REL_MASK;

impl Bound {
    pub fn new(variable: VarRef, relation: Relation, value: IntCst) -> Self {
        let var_part = u32::from(variable) << 1;
        let relation_part = relation as u32;
        let raw_value = match relation {
            Relation::LEQ => BoundValue::new_ub(value),
            Relation::GT => BoundValue::new_lb(value + 1),
        };
        let b = Bound {
            var_rel: var_part | relation_part,
            raw_value,
        };

        debug_assert_eq!(b.unpack(), (variable, relation, value));
        b
    }

    #[inline]
    pub fn variable(self) -> VarRef {
        let var_part = self.var_rel & VAR_MASK;
        let var = var_part >> 1;
        VarRef::from(var)
    }

    #[inline]
    pub fn relation(self) -> Relation {
        let rel_part = self.var_rel & REL_MASK;
        let rel = rel_part as u8;
        unsafe { transmute(rel) }
    }

    #[inline]
    pub fn value(self) -> IntCst {
        match self.relation() {
            Relation::LEQ => self.raw_value.as_ub(),
            Relation::GT => self.raw_value.as_lb() - 1,
        }
    }

    #[inline]
    pub fn affected_bound(self) -> VarBound {
        VarBound::new_raw(self.var_rel)
    }

    #[inline]
    pub fn bound_value(self) -> BoundValue {
        self.raw_value
    }

    #[inline]
    pub fn leq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::new(var.into(), Relation::LEQ, val)
    }
    #[inline]
    pub fn lt(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::leq(var, val - 1)
    }

    #[inline]
    pub fn geq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::gt(var, val - 1)
    }

    #[inline]
    pub fn gt(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::new(var.into(), Relation::GT, val)
    }

    pub fn is_true(v: BVar) -> Bound {
        Bound::geq(v, 1)
    }
    pub fn is_false(v: BVar) -> Bound {
        Bound::leq(v, 0)
    }

    pub fn made_true_by(self, event: &VarEvent) -> bool {
        let (previous, new) = match event.ev {
            DomEvent::NewLB { new, prev } => (Bound::geq(event.var, prev), Bound::geq(event.var, new)),
            DomEvent::NewUB { new, prev } => (Bound::leq(event.var, prev), Bound::leq(event.var, new)),
        };
        new.entails(self) && !previous.entails(self)
    }
    #[inline]
    pub fn made_false_by(self, event: &VarEvent) -> bool {
        (!self).made_true_by(event)
    }

    #[inline]
    pub fn entails(self, other: Bound) -> bool {
        self.var_rel == other.var_rel && self.raw_value.stronger(other.raw_value)
    }

    pub fn unpack(self) -> (VarRef, Relation, IntCst) {
        (self.variable(), self.relation(), self.value())
    }

    /// An ordering that will group bounds by (given from highest to lowest priority):
    ///  - variable
    ///  - affected bound (lower, upper)
    ///  - by value of the bound
    pub fn lexical_cmp(&self, other: &Bound) -> Ordering {
        self.cmp(other)
    }
}

impl std::ops::Not for Bound {
    type Output = Bound;

    fn not(self) -> Self::Output {
        let (var, rel, val) = self.unpack();
        Bound::new(var, !rel, val)
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
        let (var, rel, val) = self.unpack();
        write!(f, "{:?} {} {}", var, rel, val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leq(var: VarRef, val: IntCst) -> Bound {
        Bound::leq(var, val)
    }
    fn geq(var: VarRef, val: IntCst) -> Bound {
        Bound::geq(var, val)
    }

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

        let lit = Bound::leq(a, 5);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        let lit = Bound::leq(a, 0);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = Bound::leq(a, -1);
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
        let lit = Bound::leq(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
        let lit = Bound::leq(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
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
}
