use crate::collection::index_map::{IndexMap, ToIndex};
use std::ops::Index;

// todo: make the index be in [1..oo[ so that we can keep the value 0 as the representation for None

pub struct IdxHeap<K: ToIndex> {
    heap: Vec<K>,
    index: IndexMap<K, Option<usize>>,
}

impl<K: ToIndex + Copy> IdxHeap<K> {
    pub fn new_with_capacity(cap: usize) -> Self {
        IdxHeap {
            heap: Vec::with_capacity(cap),
            index: IndexMap::new(cap + 1, None),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    #[inline]
    pub fn contains(&self, key: K) -> bool {
        if self.index.raw_len() > key.to_index() {
            self.index[key].is_some()
        } else {
            false
        }
    }
    //
    //    #[inline]
    //    pub fn clear(&mut self) {
    //        self.heap.clear();
    //        self.index.clear();
    //    }

    pub fn pop<F: Fn(K, K) -> bool>(&mut self, before: F) -> Option<K> {
        if self.heap.is_empty() {
            None
        } else {
            let res = self.heap.swap_remove(0);
            self.index[res] = None;
            if !self.heap.is_empty() {
                self.sift_down(0, &before);
            }
            Some(res)
        }
    }

    pub fn insert<F: Fn(K, K) -> bool>(&mut self, key: K, before: F) {
        assert!(
            !self.contains(key),
            "Requested the insertion of a key already in the heap"
        );
        let place = self.heap.len();
        self.heap.push(key);
        self.sift_up(place, before);
    }

    pub fn update<F: Fn(K, K) -> bool>(&mut self, key: K, before: F) {
        let place = self.index[key].expect("requested an update of a non existing key.");

        self.sift_down(place, &before);
        self.sift_up(place, before);
    }

    pub fn insert_or_update<F: Fn(K, K) -> bool>(&mut self, key: K, before: F) {
        if self.contains(key) {
            self.update(key, before);
        } else {
            self.insert(key, before);
        }
    }

    //    pub fn heapify_from<F: Fn(&K, &K) -> bool>(&mut self, from: Vec<K>, before: F) {
    //        self.index.clear();
    //        self.heap = from;
    //
    //        for i in (0..self.heap.len()).rev() {
    //            self.sift_down(i, &before);
    //        }
    //    }

    fn sift_up<F: Fn(K, K) -> bool>(&mut self, mut i: usize, before: F) {
        while i > 0 {
            let p = (i - 1) >> 1;
            if before(self.heap[i], self.heap[p]) {
                self.index[self.heap[p]] = Some(i);
                self.heap.swap(i, p);
                i = p;
            } else {
                break;
            }
        }

        self.index[self.heap[i]] = Some(i);
    }

    fn sift_down<F: Fn(K, K) -> bool>(&mut self, mut i: usize, before: &F) {
        loop {
            let c = {
                let l = 2 * i + 1;
                if l >= self.heap.len() {
                    break;
                }
                let r = l + 1;
                if r < self.heap.len() && before(self.heap[r], self.heap[l]) {
                    r
                } else {
                    l
                }
            };

            if before(self.heap[c], self.heap[i]) {
                self.index[self.heap[c]] = Some(i);
                self.heap.swap(c, i);
                i = c;
            } else {
                break;
            }
        }

        self.index[self.heap[i]] = Some(i);
    }
}
