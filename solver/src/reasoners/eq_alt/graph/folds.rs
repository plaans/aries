use crate::{collections::set::RefSet, reasoners::eq_alt::relation::EqRelation};

use super::{
    traversal::{self, NodeTag},
    TaggedNode,
};

/// A fold to be used in graph traversal for nodes reachable through eq or neq relations.
pub struct EqOrNeqFold();

impl traversal::Fold<EqRelation> for EqOrNeqFold {
    fn init(&self) -> EqRelation {
        EqRelation::Eq
    }

    fn fold(&self, tag: &EqRelation, edge: &super::IdEdge) -> Option<EqRelation> {
        *tag + edge.relation
    }
}

/// A fold to be used in graph traversal for nodes reachable through eq relation only.
pub struct EqFold();

impl traversal::Fold<EmptyTag> for EqFold {
    fn init(&self) -> EmptyTag {
        EmptyTag()
    }

    fn fold(&self, _tag: &EmptyTag, edge: &super::IdEdge) -> Option<EmptyTag> {
        match edge.relation {
            EqRelation::Eq => Some(EmptyTag()),
            EqRelation::Neq => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct EmptyTag();

impl From<()> for EmptyTag {
    fn from(_value: ()) -> Self {
        EmptyTag()
    }
}

impl From<bool> for EmptyTag {
    fn from(_value: bool) -> Self {
        EmptyTag()
    }
}

impl From<EmptyTag> for bool {
    fn from(_value: EmptyTag) -> Self {
        false
    }
}

// Using EqRelation as a NodeTag requires From/To<Bool> impl
impl From<bool> for EqRelation {
    fn from(value: bool) -> Self {
        if value {
            EqRelation::Eq
        } else {
            EqRelation::Neq
        }
    }
}

impl From<EqRelation> for bool {
    fn from(value: EqRelation) -> Self {
        match value {
            EqRelation::Eq => true,
            EqRelation::Neq => false,
        }
    }
}

/// Fold which filters out TaggedNodes in set (after performing previous fold)
pub struct ReducingFold<'a, F: traversal::Fold<T>, T: NodeTag> {
    set: &'a RefSet<TaggedNode<T>>,
    fold: F,
}

impl<'a, F: traversal::Fold<T>, T: NodeTag> ReducingFold<'a, F, T> {
    pub fn new(set: &'a RefSet<TaggedNode<T>>, fold: F) -> Self {
        Self { set, fold }
    }
}

impl<'a, F: traversal::Fold<T>, T: NodeTag> traversal::Fold<T> for ReducingFold<'a, F, T> {
    fn init(&self) -> T {
        self.fold.init()
    }

    fn fold(&self, tag: &T, edge: &super::IdEdge) -> Option<T> {
        self.fold
            .fold(tag, edge)
            .filter(|new_t| !self.set.contains(TaggedNode(edge.target, *new_t)))
    }
}
