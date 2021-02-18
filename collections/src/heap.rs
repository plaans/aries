use crate::ref_store::{Ref, RefMap};
use std::num::NonZeroU32;

pub struct IdxHeap<K, P> {
    /// binary heap, the first
    heap: Vec<K>,
    index: RefMap<K, (Option<PlaceInHeap>, P)>,
}

#[derive(Copy, Clone)]
/// Encoding for the place in the heap vector. It leaves the value 0 free to allow representing
/// Option<PlaceInHeap> in 8 bytes (instead of 16 for Option<usize>)
struct PlaceInHeap(NonZeroU32);

impl Into<usize> for PlaceInHeap {
    fn into(self) -> usize {
        self.0.get() as usize - 1
    }
}
impl From<usize> for PlaceInHeap {
    fn from(x: usize) -> Self {
        unsafe { PlaceInHeap(NonZeroU32::new_unchecked(x as u32 + 1)) }
    }
}

impl<K: Ref, P: PartialOrd> Default for IdxHeap<K, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ref, P: PartialOrd> IdxHeap<K, P> {
    pub fn new() -> Self {
        IdxHeap {
            heap: Default::default(),
            index: Default::default(),
        }
    }

    /// Creates a new heap that is empty but that can handle the given number of elements.
    pub fn with_elements(num_elements: usize, default_priority: P) -> Self
    where
        P: Clone,
    {
        let mut index = RefMap::default();
        for i in 0..num_elements {
            index.insert(K::from(i), (None, default_priority.clone()))
        }
        IdxHeap {
            heap: Vec::with_capacity(num_elements),
            index,
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
        self.index.insert(key, (None, priority));
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
        self.index[key].0.is_some()
    }

    /// Returns true if the variable has been previously declared, regardless of whether it is
    /// actually in the queue.
    pub fn is_declared(&self, key: K) -> bool {
        self.index.contains(key)
    }

    pub fn peek(&self) -> Option<&K> {
        self.heap.get(0)
    }

    pub fn pop(&mut self) -> Option<K> {
        if self.is_empty() {
            None
        } else {
            let res = self.heap.swap_remove(0);
            self.index[res].0 = None;
            if !self.heap.is_empty() {
                self.sift_down(0);
            }
            Some(res)
        }
    }

    pub fn enqueue(&mut self, key: K) {
        debug_assert!(self.is_declared(key), "Key not declared");
        if !self.is_enqueued(key) {
            let place = self.heap.len();
            self.heap.push(key);
            self.sift_up(place);
        }
    }

    pub fn change_priority<F: Fn(&mut P)>(&mut self, key: K, f: F) {
        f(&mut self.index[key].1);
        self.sift_after_priority_change(key)
    }

    /// Updates the priority of all keys, **without changing their location in the heap.**
    /// For this to be correct, it should not impact the relative ordering of two items in the
    /// heap.
    pub fn change_all_priorities_in_place<F: Fn(&mut P)>(&mut self, f: F) {
        for prio in self.index.values_mut() {
            f(&mut prio.1)
        }
    }

    pub fn set_priority(&mut self, key: K, new_priority: P) {
        self.index[key].1 = new_priority;
        self.sift_after_priority_change(key);
    }

    fn sift_after_priority_change(&mut self, key: K) {
        if let Some(place) = self.index[key].0 {
            self.sift_down(place.into());
            self.sift_up(place.into());
        }
    }

    pub fn priority(&self, k: K) -> &P {
        &self.index[k].1
    }

    fn before(&self, k1: K, k2: K) -> bool {
        self.priority(k1) > self.priority(k2)
    }

    fn sift_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) >> 1;
            if self.before(self.heap[i], self.heap[p]) {
                self.index[self.heap[p]].0 = Some(PlaceInHeap::from(i));
                self.heap.swap(i, p);
                i = p;
            } else {
                break;
            }
        }
        self.index[self.heap[i]].0 = Some(PlaceInHeap::from(i));
    }

    fn sift_down(&mut self, mut i: usize) {
        fn below_left(idx: usize) -> usize {
            (idx << 1) + 1
        }
        let len = self.heap.len();
        let key = self.heap[i];

        let mut child = below_left(i);
        while child < len - 1 {
            let prio = &self.index[key].1;
            let left = self.heap[child];
            let p_left = self.priority(left);
            let right = self.heap[child + 1];
            let p_right = self.priority(right);
            let p: &P;
            let child_key;
            if p_right > p_left {
                child += 1;
                p = p_right;
                child_key = right;
            } else {
                p = p_left;
                child_key = left;
            }
            if p > prio {
                self.heap[i] = child_key;
                self.index[child_key].0 = Some(PlaceInHeap::from(i));
            } else {
                self.index[key].0 = Some(PlaceInHeap::from(i));
                self.heap[i] = key;
                return;
            }
            i = child;
            child = below_left(child);
        }
        if child == len - 1 {
            let child_key = self.heap[child];
            if self.priority(child_key) > self.priority(key) {
                self.heap[i] = child_key;
                self.index[child_key].0 = Some(PlaceInHeap::from(i));
                i = child;
            }
        }
        self.index[key].0 = Some(PlaceInHeap::from(i));
        self.heap[i] = key;
    }
}
