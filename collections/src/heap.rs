use crate::heap::Entry::{In, Out};
use crate::ref_store::{Ref, RefMap};

pub struct IdxHeap<K, P> {
    /// binary heap, the first
    heap: Vec<(K, P)>,
    index: RefMap<K, Entry<P>>,
}

enum Entry<P> {
    In(PlaceInHeap),
    Out(P),
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
/// Encoding for the place in the heap vector. It leaves the value 0 free to allow representing
/// Option<PlaceInHeap> in 8 bytes (instead of 16 for Option<usize>)
struct PlaceInHeap(usize);

impl PlaceInHeap {
    const ROOT: PlaceInHeap = PlaceInHeap(0);

    pub fn above(self) -> PlaceInHeap {
        debug_assert!(self.0 > 0);
        PlaceInHeap((self.0 - 1) >> 1)
    }

    pub fn left(self) -> PlaceInHeap {
        PlaceInHeap(self.0 * 2 + 1)
    }

    pub fn right(self) -> PlaceInHeap {
        PlaceInHeap(self.0 * 2 + 2)
    }
}

impl From<usize> for PlaceInHeap {
    fn from(x: usize) -> Self {
        PlaceInHeap(x)
    }
}
impl From<PlaceInHeap> for usize {
    fn from(p: PlaceInHeap) -> Self {
        p.0
    }
}
impl<T> std::ops::Index<PlaceInHeap> for Vec<T> {
    type Output = T;

    fn index(&self, index: PlaceInHeap) -> &Self::Output {
        &self[usize::from(index)]
    }
}
impl<T> std::ops::IndexMut<PlaceInHeap> for Vec<T> {
    fn index_mut(&mut self, index: PlaceInHeap) -> &mut Self::Output {
        &mut self[usize::from(index)]
    }
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
        self.heap.first().map(|e| &e.0)
    }

    pub fn pop(&mut self) -> Option<K> {
        if self.is_empty() {
            None
        } else {
            let (key, prio) = self.heap.swap_remove(0);
            self.index[key] = Out(prio);
            if !self.heap.is_empty() {
                self.sift_down(PlaceInHeap::ROOT);
            }
            Some(key)
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
                self.heap.push((key, *prio));
                self.sift_up(place);
            }
        }
    }

    pub fn change_priority<F: Fn(&mut P)>(&mut self, key: K, f: F) {
        match &mut self.index[key] {
            In(loc) => {
                let loc = *loc;
                f(&mut self.heap[loc].1);
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
                In(loc) => f(&mut self.heap[*loc].1),
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
            In(p) => self.heap[p].1,
            Out(p) => p,
        }
    }
    //
    // fn before(&self, k1: K, k2: K) -> bool {
    //     self.priority(k1) > self.priority(k2)
    // }

    fn sift_up(&mut self, mut i: PlaceInHeap)
    where
        P: Copy,
    {
        let (key, prio) = self.heap[i];
        while i > PlaceInHeap::ROOT {
            let p = i.above();
            let (above_key, above_prio) = self.heap[p];
            if above_prio < prio {
                self.index[above_key] = In(i);
                self.heap.swap(usize::from(i), usize::from(p));
                i = p;
            } else {
                break;
            }
        }
        self.index[key] = In(i);
    }

    fn free(&self) -> PlaceInHeap {
        self.heap.len().into()
    }

    fn sift_down(&mut self, mut i: PlaceInHeap) {
        let len = self.free();
        let (key, prio) = self.heap[i];
        loop {
            let c = {
                let l = i.left();
                if l >= len {
                    break;
                }
                let r = i.right();
                if r < len && self.heap[r].1 > self.heap[l].1 {
                    r
                } else {
                    l
                }
            };

            if self.heap[c].1 > prio {
                self.index[self.heap[c].0] = In(i);
                self.heap.swap(c.into(), i.into());
                i = c;
            } else {
                break;
            }
        }

        self.index[key] = In(i);
    }
}
