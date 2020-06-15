use crate::collection::id_map::IdMap;
use std::num::NonZeroUsize;
// todo: make the index be in [1..oo[ so that we can keep the value 0 as the representation for None

pub struct IdxHeap<K> {
    /// binary heap, the first
    heap: Vec<K>,
    index: IdMap<K, PlaceInHeap>,
}

#[derive(Copy, Clone)]
/// Encoding for the place in the heap vector. It leaves the value 0 free to allo reprenting
/// Option<PlaceInHeap> in 8 bytes (instead of 16 for Option<usize>)
struct PlaceInHeap(NonZeroUsize);

impl Into<usize> for PlaceInHeap {
    fn into(self) -> usize {
        self.0.get() - 1
    }
}
impl From<usize> for PlaceInHeap {
    fn from(x: usize) -> Self {
        unsafe { PlaceInHeap(NonZeroUsize::new_unchecked(x + 1)) }
    }
}

impl<K: Into<usize> + Copy> IdxHeap<K> {
    pub fn new_with_capacity(cap: usize) -> Self {
        IdxHeap {
            heap: Vec::with_capacity(cap),
            index: IdMap::new(),
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
        self.index.contains_key(key)
    }
    //
    //    #[inline]
    //    pub fn clear(&mut self) {
    //        self.heap.clear();
    //        self.index.clear();
    //    }

    pub fn pop<F: Fn(K, K) -> bool>(&mut self, before: F) -> Option<K> {
        if self.is_empty() {
            None
        } else {
            let res = self.heap.swap_remove(0);
            self.index.remove(res);
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
        let &place = self.index.get(key).expect("requested an update of a non existing key.");

        self.sift_down(place.into(), &before);
        self.sift_up(place.into(), before);
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
                self.index.insert(self.heap[p], PlaceInHeap::from(i));
                self.heap.swap(i, p);
                i = p;
            } else {
                break;
            }
        }
        self.index.insert(self.heap[i], PlaceInHeap::from(i));
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
                self.index.insert(self.heap[c], PlaceInHeap::from(i));
                self.heap.swap(c, i);
                i = c;
            } else {
                break;
            }
        }

        self.index.insert(self.heap[i], PlaceInHeap::from(i));
    }
}
