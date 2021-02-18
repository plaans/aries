use crate::heap::Entry::{In, Out};
use crate::ref_store::{Ref, RefMap};
use std::cmp::Ordering;

#[derive(Copy, Clone)]
struct HeapEntry<K, P> {
    key: K,
    prio: P,
}
impl<K, P> HeapEntry<K, P> {
    pub fn new(key: K, prio: P) -> Self {
        HeapEntry { key, prio }
    }
}
impl<K, P: PartialEq> PartialEq for HeapEntry<K, P> {
    fn eq(&self, other: &Self) -> bool {
        self.prio == other.prio
    }
}
impl<K, P: PartialOrd> PartialOrd for HeapEntry<K, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.prio.partial_cmp(&other.prio)
    }
}

pub struct IdxHeap<K, P> {
    /// binary heap, the first
    heap: Vec<HeapEntry<K, P>>,
    index: RefMap<K, Entry<P>>,
}

enum Entry<P> {
    In(PlaceInHeap),
    Out(P),
}

type PlaceInHeap = usize;
fn above(i: usize) -> usize {
    debug_assert!(i > 0);
    (i - 1) >> 1
}
#[inline]
fn below_left(i: usize) -> usize {
    (i << 1) + 1
}
#[inline]
fn below_right(i: usize) -> usize {
    (i << 1) + 2
}

impl<K: Ref, P: PartialOrd + Copy> Default for IdxHeap<K, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ref, P: PartialOrd + Copy> IdxHeap<K, P> {
    pub fn new() -> Self {
        IdxHeap {
            heap: Default::default(),
            index: Default::default(),
        }
    }

    pub fn num_enqueued_elements(&self) -> usize {
        self.heap.len()
    }

    /// Record a new element that is NOT added in the queue.
    /// The element is assigned the given priority.
    pub fn declare_element(&mut self, key: K, priority: P)
    where
        K: From<usize>,
    {
        assert!(!self.index.contains(key));
        self.index.insert(key, Out(priority));
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_
    where
        K: From<usize>,
    {
        self.index.keys()
    }

    pub fn is_enqueued(&self, key: K) -> bool {
        debug_assert!(self.is_declared(key), "Variable is not declared");
        matches!(self.index[key], In(_))
    }

    /// Returns true if the variable has been previously declared, regardless of whether it is
    /// actually in the queue.
    pub fn is_declared(&self, key: K) -> bool {
        self.index.contains(key)
    }

    pub fn peek(&self) -> Option<&K> {
        self.heap.first().map(|e| &e.key)
    }

    pub fn pop(&mut self) -> Option<K> {
        if self.is_empty() {
            None
        } else {
            let head = self.heap.swap_remove(0);
            self.index[head.key] = Out(head.prio);
            if !self.heap.is_empty() {
                self.sift_down(0);
            }
            Some(head.key)
        }
    }

    pub fn enqueue(&mut self, key: K)
    where
        P: Copy,
    {
        debug_assert!(self.is_declared(key), "Key not declared");
        match &self.index[key] {
            In(_) => {
                // already in queue, do nothing
            }
            Out(prio) => {
                let place = self.free();
                self.heap.push(HeapEntry::new(key, *prio));
                self.sift_up(place);
            }
        }
    }

    pub fn change_priority<F: Fn(&mut P)>(&mut self, key: K, f: F) {
        match &mut self.index[key] {
            In(loc) => {
                let loc = *loc;
                f(&mut self.heap[loc].prio);
                self.sift_after_priority_change(loc);
            }
            Out(p) => f(p),
        }
    }

    /// Updates the priority of all keys, **without changing their location in the heap.**
    /// For this to be correct, it should not impact the relative ordering of two items in the
    /// heap.
    pub fn change_all_priorities_in_place<F: Fn(&mut P)>(&mut self, f: F) {
        for entry in self.index.values_mut() {
            match entry {
                In(loc) => f(&mut self.heap[*loc].prio),
                Out(p) => f(p),
            }
        }
    }

    pub fn set_priority(&mut self, key: K, new_priority: P) {
        self.change_priority(key, |p| *p = new_priority);
    }

    fn sift_after_priority_change(&mut self, place: PlaceInHeap) {
        self.sift_down(place);
        self.sift_up(place);
    }

    pub fn priority(&self, k: K) -> P {
        match self.index[k] {
            In(p) => self.heap[p].prio,
            Out(p) => p,
        }
    }

    fn sift_up(&mut self, mut i: PlaceInHeap)
    where
        P: Copy,
    {
        let pivot = self.heap[i];
        while i > 0 {
            let p = above(i);
            let above = self.heap[p];
            if above < pivot {
                self.index[above.key] = In(i);
                self.heap.swap(usize::from(i), usize::from(p));
                i = p;
            } else {
                break;
            }
        }
        self.index[pivot.key] = In(i);
    }

    fn free(&self) -> PlaceInHeap {
        self.heap.len()
    }

    fn sift_down(&mut self, mut i: PlaceInHeap) {
        let len = self.free();
        let pivot = self.heap[i];
        loop {
            let l = below_left(i);
            let r = below_right(i);
            let (c, val) = if r < len {
                let left = self.heap[l];
                let right = self.heap[r];
                if right > left {
                    (r, right)
                } else {
                    (l, left)
                }
            } else if l < len {
                (l, self.heap[l])
            } else {
                break;
            };

            if val > pivot {
                self.index[val.key] = In(i);
                self.heap[i] = val;
                i = c;
            } else {
                break;
            }
        }
        self.index[pivot.key] = In(i);
        self.heap[i] = pivot;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::prelude::StdRng;
    use rand::{Rng, SeedableRng};

    const N: usize = 100;

    #[test]
    fn test_heap_insertion_removal() {
        let mut rng = StdRng::seed_from_u64(79837224973);
        let mut heap = IdxHeap::new();

        let mut priorities = Vec::new();

        fn eq(a: f64, b: f64) -> bool {
            f64::abs(a - b) < f64::EPSILON
        }

        for i in 0..N {
            let prio = rng.gen_range(-100..100) as f64;
            priorities.push(prio);
            heap.declare_element(i, prio);
            assert!(eq(heap.priority(i), prio));
        }

        let remove_n = |heap: &mut IdxHeap<usize, f64>, n: usize| -> Vec<usize> {
            let mut removed = Vec::new();
            let first = heap.pop().unwrap();
            removed.push(first);
            let mut previous_best = heap.priority(first);
            assert!(eq(previous_best, priorities[first]));
            for _ in 1..n {
                let next = heap.pop().unwrap();
                let p = heap.priority(next);
                assert!(eq(p, priorities[next]));
                assert!(p <= previous_best, "p: {}   prev:{}", p, previous_best);
                previous_best = p;
                removed.push(next);
            }

            removed
        };
        let insert_all = |heap: &mut IdxHeap<usize, f64>, to_insert: &[usize]| {
            for elt in to_insert {
                heap.enqueue(*elt);
            }
        };

        let entries: Vec<usize> = (0..N).collect();
        insert_all(&mut heap, &entries);

        // remove half elements
        let out = remove_n(&mut heap, N / 2);
        for elt in 0..N {
            assert_eq!(heap.is_enqueued(elt), !out.contains(&elt));
        }
        insert_all(&mut heap, &out);
        remove_n(&mut heap, N);
        assert!(heap.pop().is_none());
        assert!(heap.is_empty());
    }
}
