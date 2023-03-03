use crate::theory::contraint_db::Enabler;
use crate::theory::{Timepoint, W};
use aries_core::{BoundValueAdd, Lit, SignedVar};

/// An edge in the STN, representing the constraint `target - source <= weight`
/// An edge can be either in canonical form or in negated form.
/// Given to edges (tgt - src <= w) and (tgt -src > w) one will be in canonical form and
/// the other in negated form.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Edge {
    pub source: Timepoint,
    pub target: Timepoint,
    pub weight: W,
}

impl Edge {
    #[allow(unused)]
    pub fn new(source: Timepoint, target: Timepoint, weight: W) -> Edge {
        Edge { source, target, weight }
    }

    /// The negated version of this edge that is valid iff this one is invalid.
    #[allow(unused)]
    pub fn negated(&self) -> Self {
        // not(b - a <= 6)
        //   = b - a > 6
        //   = a -b < -6
        //   = a - b <= -7
        //
        // not(a - b <= -7)
        //   = a - b > -7
        //   = b - a < 7
        //   = b - a <= 6
        Edge {
            source: self.target,
            target: self.source,
            weight: -self.weight - 1,
        }
    }
}

/// A `Propagator` represents the fact that an update on the `source` bound
/// should be reflected on the `target` bound.
///
/// From a classical STN edge `source -- weight --> target` there will be two `Propagator`s:
///   - ub(source) = X   implies   ub(target) <= X + weight
///   - lb(target) = X   implies   lb(source) >= X - weight
#[derive(Clone, Debug)]
pub(crate) struct Propagator {
    pub source: SignedVar,
    pub target: SignedVar,
    pub weight: BoundValueAdd,
    /// Literals describing when the propagator should be enabled.
    pub enabler: Enabler,
}

/// A `PropagatorGroup` represents the fact that an update on the `source` bound
/// should be reflected on the `target` bound when some conditions holds.
/// It represents a set of `Propagator`s that differ only by their enabling conditions.
#[derive(Clone, Debug)]
pub(crate) struct PropagatorGroup {
    pub source: SignedVar,
    pub target: SignedVar,
    pub weight: BoundValueAdd,
    /// Non-empty if the constraint active (participates in propagation)
    /// If the enabler is Lit::TRUE, then the constraint can be assumed to be always active
    pub enabler: Option<Enabler>,
    /// A set of potential enablers for this constraint.
    /// The edge becomes active once one of its enablers becomes true
    pub enablers: Vec<Enabler>,
}

/// Represents an edge together with a particular propagation direction:
///  - forward (source to target)
///  - backward (target to source)
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub(crate) struct PropagatorId(u32);

impl From<PropagatorId> for usize {
    fn from(e: PropagatorId) -> Self {
        e.0 as usize
    }
}
impl From<usize> for PropagatorId {
    fn from(u: usize) -> Self {
        PropagatorId(u as u32)
    }
}
impl From<PropagatorId> for u32 {
    fn from(e: PropagatorId) -> Self {
        e.0
    }
}
impl From<u32> for PropagatorId {
    fn from(u: u32) -> Self {
        PropagatorId(u)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PropagatorTarget {
    pub target: SignedVar,
    pub weight: BoundValueAdd,
    /// Literal that is true if and only if the edge must be present in the network.
    /// Note that handling of optional variables might allow and edge to propagate even it is not known
    /// to be present yet.
    pub presence: Lit,
}
