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
pub struct EdgeId(u32);
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

    /// Id of the forward (from source to target) view of this edge
    pub(in crate::theory) fn forward(self) -> DirEdge {
        DirEdge::forward(self)
    }

    /// Id of the backward view (from target to source) of this edge
    pub(in crate::theory) fn backward(self) -> DirEdge {
        DirEdge::backward(self)
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
    pub fn new(source: Timepoint, target: Timepoint, weight: W) -> Edge {
        Edge { source, target, weight }
    }

    pub fn is_negated(&self) -> bool {
        !self.is_canonical()
    }

    pub fn is_canonical(&self) -> bool {
        self.source < self.target || self.source == self.target && self.weight >= 0
    }

    // not(b - a <= 6)
    //   = b - a > 6
    //   = a -b < -6
    //   = a - b <= -7
    //
    // not(a - b <= -7)
    //   = a - b > -7
    //   = b - a < 7
    //   = b - a <= 6
    pub fn negated(&self) -> Self {
        Edge {
            source: self.target,
            target: self.source,
            weight: -self.weight - 1,
        }
    }
}

/// A directional constraint representing the fact that an update on the `source` bound
/// should be reflected on the `target` bound.
///
/// From a classical STN edge `source -- weight --> target` there will be two directional constraints:
///   - ub(source) = X   implies   ub(target) <= X + weight
///   - lb(target) = X   implies   lb(source) >= X - weight
#[derive(Clone, Debug)]
pub struct DirConstraint {
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
impl DirConstraint {
    /// source <= X   =>   target <= X + weight
    pub fn forward(edge: Edge) -> DirConstraint {
        DirConstraint {
            source: VarBound::ub(edge.source),
            target: VarBound::ub(edge.target),
            weight: BoundValueAdd::on_ub(edge.weight),
            enabler: None,
            enablers: vec![],
        }
    }

    /// target >= X   =>   source >= X - weight
    pub fn backward(edge: Edge) -> DirConstraint {
        DirConstraint {
            source: VarBound::lb(edge.target),
            target: VarBound::lb(edge.source),
            weight: BoundValueAdd::on_lb(-edge.weight),
            enabler: None,
            enablers: vec![],
        }
    }

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
pub(crate) struct DirEdge(u32);

impl DirEdge {
    /// Forward view of the given edge
    pub fn forward(e: EdgeId) -> Self {
        DirEdge(u32::from(e) << 1)
    }

    /// Backward view of the given edge
    pub fn backward(e: EdgeId) -> Self {
        DirEdge((u32::from(e) << 1) + 1)
    }

    #[allow(unused)]
    pub fn is_forward(self) -> bool {
        (u32::from(self) & 0x1) == 0
    }

    /// The edge underlying this projection
    pub fn edge(self) -> EdgeId {
        EdgeId::from(self.0 >> 1)
    }
}
impl From<DirEdge> for usize {
    fn from(e: DirEdge) -> Self {
        e.0 as usize
    }
}
impl From<usize> for DirEdge {
    fn from(u: usize) -> Self {
        DirEdge(u as u32)
    }
}
impl From<DirEdge> for u32 {
    fn from(e: DirEdge) -> Self {
        e.0
    }
}
impl From<u32> for DirEdge {
    fn from(u: u32) -> Self {
        DirEdge(u)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct EdgeTarget {
    pub target: VarBound,
    pub weight: BoundValueAdd,
    /// Literal that is true if and only if the edge must be present in the network.
    /// Note that handling of optional variables might allow and edge to propagate even it is not known
    /// to be present yet.
    pub presence: Lit,
}
