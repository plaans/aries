use crate::bounds::var_bound::VarBound;
use crate::bounds::BoundValue;
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
/// assert_eq!(y_geq_5.relation(), Relation::Gt);
/// assert_eq!(y_geq_5.value(), 4);
/// assert_eq!(y_geq_5.unpack(), (y, Relation::Gt, 4));
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
/// let mut bounds = vec![Bound::geq(y, 4), Bound::geq(x,1), Bound::leq(x, 3), Bound::leq(x, 4), Bound::leq(x, 6), Bound::geq(x,2)];
/// bounds.sort();
/// assert_eq!(bounds, vec![Bound::geq(x,2), Bound::geq(x,1), Bound::leq(x, 3), Bound::leq(x, 4), Bound::leq(x, 6), Bound::geq(y, 4)]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Bound {
    /// Union of the variable (highest 31 bits) and the relation (lowest bit)
    /// This encoding allows:
    ///  - to very efficiently check whether two bounds have the same `(variable, relation)` part
    ///    which is one of the critical operation in `entails`.
    ///  - to use as an index in a table: each variable will have two slots: one of the LEQ relation
    ///    and one for the GT relation
    //  TODO: use a VarBound
    pub(in crate::bounds) var_rel: u32,
    /// +/- the value of the relation. The value of a GT relation is negated before being stored.
    /// This design allows to test entailment without testing the relation of the Bound
    pub(in crate::bounds) raw_value: BoundValue,
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum Relation {
    Gt = 0,
    Leq = 1,
}

impl std::ops::Not for Relation {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        unsafe { transmute((self as u8) ^ 0x1) }
    }
}

impl std::fmt::Display for Relation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Relation::Leq => write!(f, "<="),
            Relation::Gt => write!(f, ">"),
        }
    }
}

const REL_MASK: u32 = 0x1;
const VAR_MASK: u32 = !REL_MASK;

impl Bound {
    /// A literal that is always true. It is defined by by stating that the special variable [VarRef::ZERO] is
    /// lesser than or equal to 0, which is always true.
    pub const TRUE: Bound = Bound::new(VarRef::ZERO, Relation::Leq, 0);
    pub const FALSE: Bound = Bound::TRUE.not();

    #[inline]
    pub const fn from_parts(var_bound: VarBound, value: BoundValue) -> Self {
        Bound {
            var_rel: var_bound.to_u32(),
            raw_value: value,
        }
    }

    #[inline]
    pub const fn new(variable: VarRef, relation: Relation, value: IntCst) -> Self {
        let var_part = variable.to_u32() << 1;
        let relation_part = relation as u32;
        let raw_value = match relation {
            Relation::Leq => BoundValue::ub(value),
            Relation::Gt => BoundValue::lb(value + 1),
        };
        Bound {
            var_rel: var_part | relation_part,
            raw_value,
        }
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
            Relation::Leq => self.raw_value.as_ub(),
            Relation::Gt => self.raw_value.as_lb() - 1,
        }
    }

    #[inline]
    pub fn affected_bound(self) -> VarBound {
        VarBound::from_raw(self.var_rel)
    }

    #[inline]
    pub fn bound_value(self) -> BoundValue {
        self.raw_value
    }

    #[inline]
    pub fn leq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::new(var.into(), Relation::Leq, val)
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
        Bound::new(var.into(), Relation::Gt, val)
    }

    pub fn is_true(v: BVar) -> Bound {
        Bound::geq(v, 1)
    }
    pub fn is_false(v: BVar) -> Bound {
        Bound::leq(v, 0)
    }

    #[inline]
    pub const fn not(self) -> Self {
        Bound {
            var_rel: self.var_rel ^ 0x1,
            raw_value: self.raw_value.neg(),
        }
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

    #[inline]
    fn not(self) -> Self::Output {
        self.not()
    }
}

impl From<BVar> for Bound {
    fn from(v: BVar) -> Self {
        v.true_lit()
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
