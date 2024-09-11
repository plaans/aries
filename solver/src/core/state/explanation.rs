use crate::backtrack::EventIndex;
use crate::core::state::{Domains, InferenceCause};
use crate::core::Lit;
use std::collections::BinaryHeap;

/// Builder for a conjunction of literals that make the explained literal true
#[derive(Clone, Debug)]
pub struct Explanation {
    pub lits: Vec<Lit>,
}
impl Explanation {
    pub fn new() -> Self {
        Explanation { lits: Vec::new() }
    }
    pub fn with_capacity(n: usize) -> Self {
        Explanation {
            lits: Vec::with_capacity(n),
        }
    }
    pub fn reserve(&mut self, additional: usize) {
        self.lits.reserve(additional)
    }
    pub fn push(&mut self, lit: Lit) {
        self.lits.push(lit)
    }
    pub fn pop(&mut self) -> Option<Lit> {
        self.lits.pop()
    }

    pub fn clear(&mut self) {
        self.lits.clear();
    }

    pub fn literals(&self) -> &[Lit] {
        &self.lits
    }
}
impl Default for Explanation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Explainer {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation);
}

/// A provides an explainer for a standalone theory. useful for testing purposes.
#[cfg(test)]
pub struct SingleTheoryExplainer<'a, T: crate::reasoners::Theory>(pub &'a mut T);

#[cfg(test)]
impl<'a, T: crate::reasoners::Theory> Explainer for SingleTheoryExplainer<'a, T> {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation) {
        assert_eq!(cause.writer, self.0.identity());
        self.0.explain(literal, cause, model, explanation)
    }
}

/// A priority queue aimed at producing explanations.
///
/// The queue contains a set of entailed literals, together with the index of the event
/// that entailed them.
///
/// The queue allows iterating through those literals from the most recent to the oldest while removing duplicates.
#[derive(Clone, Default)]
pub(crate) struct ExplanationQueue {
    heap: BinaryHeap<InQueueLit>,
}

impl ExplanationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
    pub fn push(&mut self, cause: EventIndex, lit: Lit) {
        self.heap.push(InQueueLit { cause, lit })
    }

    /// remove the next event to process.
    /// Note that this method will collapse any two event where one subsumes the other.
    pub fn pop(&mut self) -> Option<(Lit, EventIndex)> {
        if self.is_empty() {
            return None;
        }
        let mut l = self.heap.pop().unwrap();
        // The queue might contain more than one reference to the same event.
        // Due to the priority of the queue, they necessarily contiguous
        while let Some(next) = self.heap.peek() {
            // check if next event is the same one
            if next.cause == l.cause {
                // they are the same, pop it from the queue
                let l2 = self.heap.pop().unwrap();
                // of the two literal, keep the most general one
                if l2.lit.entails(l.lit) {
                    l = l2;
                } else {
                    // l is more general, keep it and continue
                    debug_assert!(l.lit.entails(l2.lit));
                }
            } else {
                // next is on a different event, we can proceed
                break;
            }
        }
        Some((l.lit, l.cause))
    }

    pub fn clear(&mut self) {
        self.heap.clear()
    }
}

/// A literal in an explanation queue
#[derive(Copy, Clone, Debug)]
struct InQueueLit {
    cause: EventIndex,
    lit: Lit,
}
impl PartialEq for InQueueLit {
    fn eq(&self, other: &Self) -> bool {
        self.cause == other.cause
    }
}
impl Eq for InQueueLit {}
impl Ord for InQueueLit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cause.cmp(&other.cause)
    }
}
impl PartialOrd for InQueueLit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
