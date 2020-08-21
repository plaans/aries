use crate::ref_store::RefVec;
use std::num::NonZeroU32;

pub struct IdxHeap<K, P> {
    /// binary heap, the first
    heap: Vec<K>,
    index: RefVec<K, (Option<PlaceInHeap>, P)>,
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

impl<K: Into<usize> + Copy, P: PartialOrd> IdxHeap<K, P> {
    /// Creates a new heap that is empty but that can handle the given number of elements.
    pub fn with_elements(num_elements: usize, default_priority: P) -> Self
    where
        P: Clone,
    {
        IdxHeap {
            heap: Vec::with_capacity(num_elements),
            index: RefVec::with_values(num_elements, (None, default_priority)),
        }
    }

    pub fn num_recorded_elements(&self) -> usize {
        self.index.len()
    }

    pub fn num_enqueued_elements(&self) -> usize {
        self.heap.len()
    }

    /// Record a new element that is NOT added in the queue.
    /// This element should be the next that is not recorded yet.
    /// `usize::from(key) == self.num_recorded_elements()`
    pub fn record_element(&mut self, key: K, priority: P)
    where
        K: From<usize>,
    {
        let k2 = self.index.push((None, priority));
        debug_assert_eq!(key.into(), k2.into());
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = K>
    where
        K: From<usize>,
    {
        self.index.keys()
    }

    pub fn contains(&self, key: K) -> bool {
        self.index[key].0.is_some()
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
        if !self.contains(key) {
            let place = self.heap.len();
            self.heap.push(key);
            self.sift_up(place);
        }
    }

    pub fn change_priority<F: Fn(&mut P)>(&mut self, key: K, f: F) {
        f(&mut self.index[key].1);
        self.sift_after_priority_change(key)
    }

    /// Updates the priority of a given key without changing its place in the heap.
    ///
    /// # Safety
    /// Calling this function might result in a corrupted heap if some relative orders between elements
    /// of the heap are changed.
    pub unsafe fn change_priority_unchecked<F: Fn(&mut P)>(&mut self, key: K, f: F) {
        f(&mut self.index[key].1);
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
        loop {
            let c = {
                let l = 2 * i + 1;
                if l >= self.heap.len() {
                    break;
                }
                let r = l + 1;
                if r < self.heap.len() && self.before(self.heap[r], self.heap[l]) {
                    r
                } else {
                    l
                }
            };

            if self.before(self.heap[c], self.heap[i]) {
                self.index[self.heap[c]].0 = Some(PlaceInHeap::from(i));
                self.heap.swap(c, i);
                i = c;
            } else {
                break;
            }
        }

        self.index[self.heap[i]].0 = Some(PlaceInHeap::from(i));
    }
}
