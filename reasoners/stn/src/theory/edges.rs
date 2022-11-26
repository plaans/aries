use crate::theory::contraint_db::Enabler;
use crate::theory::{Timepoint, W};
use aries_core::{BoundValueAdd, Lit, VarBound};

/// A unique identifier for an edge in the STN.
/// An edge and its negation share the same `base_id` but differ by the `is_negated` property.
///
/// For instance, valid edge ids:
///  -  `a - b <= 10`
///    - base_id: 3
///    - negated: false
///  - `a - b > 10`       # negation of the previous one
///    - base_id: 3        # same
///    - negated: true     # inverse
///  - `a - b <= 20`      # unrelated
///    - base_id: 4
///    - negated: false
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct EdgeId(u32); // FIXME: EdgeIds should disappear (the new way to add propagators makes them an invalid abstraction)
impl EdgeId {
    #[inline]
    pub fn new(base_id: u32, negated: bool) -> EdgeId {
        if negated {
            EdgeId((base_id << 1) + 1)
        } else {
            EdgeId(base_id << 1)
        }
    }

    #[inline]
    pub fn base_id(&self) -> u32 {
        self.0 >> 1
    }

    #[inline]
    pub fn is_negated(&self) -> bool {
        self.0 & 0x1 == 1
    }
}

impl std::ops::Not for EdgeId {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        EdgeId(self.0 ^ 0x1)
    }
}

impl From<EdgeId> for u32 {
    fn from(e: EdgeId) -> Self {
        e.0
    }
}
impl From<u32> for EdgeId {
    fn from(id: u32) -> Self {
        EdgeId(id)
    }
}

impl From<EdgeId> for usize {
    fn from(e: EdgeId) -> Self {
        e.0 as usize
    }
}
impl From<usize> for EdgeId {
    fn from(id: usize) -> Self {
        EdgeId(id as u32)
    }
}

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
    pub fn is_canonical(&self) -> bool {
        self.source < self.target || self.source == self.target && self.weight >= 0
    }
}

/// A `Propagator` represents the fact that an update on the `source` bound
/// should be reflected on the `target` bound.
///
/// From a classical STN edge `source -- weight --> target` there will be two `Propagator`s:
///   - ub(source) = X   implies   ub(target) <= X + weight
///   - lb(target) = X   implies   lb(source) >= X - weight
/// TODO: clarify relationship with `Propagator` (hint this on is a single 'user facing' propagator)
#[derive(Clone, Debug)]
pub(crate) struct SPropagator {
    pub source: VarBound,
    pub target: VarBound,
    pub weight: BoundValueAdd,
    /// Non-empty if the constraint active (participates in propagation)
    /// If the enabler is Lit::TRUE, then the constraint can be assumed to be always active
    pub enabler: Enabler,
}

/// A `Propagator` represents the fact that an update on the `source` bound
/// should be reflected on the `target` bound.
///
/// From a classical STN edge `source -- weight --> target` there will be two `Propagator`s:
///   - ub(source) = X   implies   ub(target) <= X + weight
///   - lb(target) = X   implies   lb(source) >= X - weight
#[derive(Clone, Debug)]
pub(crate) struct Propagator {
    pub source: VarBound,
    pub target: VarBound,
    pub weight: BoundValueAdd,
    /// Non-empty if the constraint active (participates in propagation)
    /// If the enabler is Lit::TRUE, then the constraint can be assumed to be always active
    pub enabler: Option<Enabler>,
    /// A set of potential enablers for this constraint.
    /// The edge becomes active once one of its enablers becomes true
    pub enablers: Vec<Enabler>,
}
impl Propagator {
    pub fn as_edge(&self) -> Edge {
        if self.source.is_ub() {
            debug_assert!(self.target.is_ub());
            Edge {
                source: self.source.variable(),
                target: self.target.variable(),
                weight: self.weight.as_ub_add(),
            }
        } else {
            debug_assert!(self.target.is_lb());
            Edge {
                source: self.target.variable(),
                target: self.source.variable(),
                weight: -self.weight.as_lb_add(),
            }
        }
    }
}

/// Represents an edge together with a particular propagation direction:
///  - forward (source to target)
///  - backward (target to source)
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub(crate) struct PropagatorId(u32);

impl PropagatorId {
    /// The edge underlying this projection
    pub fn edge(self) -> EdgeId {
        EdgeId::from(self.0 >> 1)
    }
}
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
    pub target: VarBound,
    pub weight: BoundValueAdd,
    /// Literal that is true if and only if the edge must be present in the network.
    /// Note that handling of optional variables might allow and edge to propagate even it is not known
    /// to be present yet.
    pub presence: Lit,
}
