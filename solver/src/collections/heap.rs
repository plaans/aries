use crate::collections::heap::Entry::{In, Out};
use crate::collections::ref_store::{Ref, RefMap};
use core::ptr;
use std::cmp::Ordering;
use std::mem::ManuallyDrop;

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

#[derive(Clone)]
pub struct IdxHeap<K, P> {
    /// binary heap, the first
    heap: Vec<HeapEntry<K, P>>,
    index: RefMap<K, Entry<P>>,
}

#[derive(Clone, Debug, PartialEq)]
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

    /// Returns all variables currently in the heap.
    pub fn enqueued_variables(&self) -> impl Iterator<Item = K> + '_ {
        self.heap.iter().map(|e| e.key)
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
        if let Some(first) = self.heap.first().copied() {
            self.index[first.key] = Out(first.prio);

            let initial_len = self.heap.len();
            let last = self.heap.pop().unwrap();
            if initial_len == 1 {
                // there was a single element which is the one we had to remove
            } else {
                debug_assert!(initial_len >= 2);
                unsafe {
                    let hole = Hole::new_with_element(&mut self.heap, &mut self.index.entries, 0, last);
                    // Self::sift_to_bottom_then_up(hole); // other option that should allow less comparisons
                    Self::sift_hole_down(hole);
                }
            }

            Some(first.key)
        } else {
            None
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

    fn sift_up(&mut self, i: PlaceInHeap)
    where
        P: Copy,
    {
        unsafe {
            let hole = self.make_hole(i);
            Self::sift_hole_up(hole);
        }
    }

    fn sift_hole_up(mut hole: Hole<K, P>)
    where
        P: Copy,
    {
        unsafe {
            while hole.pos > 0 {
                let parent = above(hole.pos);
                if hole.element() > hole.get(parent) {
                    hole.move_to(parent);
                } else {
                    break;
                }
            }
        }
    }

    fn free(&self) -> PlaceInHeap {
        self.heap.len()
    }

    unsafe fn make_hole(&mut self, pos: PlaceInHeap) -> Hole<K, P> {
        Hole::new(&mut self.heap, &mut self.index.entries, pos)
    }

    fn sift_down(&mut self, i: PlaceInHeap) {
        unsafe { Self::sift_hole_down(self.make_hole(i)) }
    }
    fn sift_hole_down(mut hole: Hole<K, P>) {
        let len = hole.data.len();
        unsafe {
            let mut child = below_left(hole.pos);
            while child < len - 1 {
                debug_assert_eq!(child, below_left(hole.pos));
                // we have both a left and a right child
                // select as child the one with the greatest priority
                child += (hole.get(child) < hole.get(child + 1)) as usize;
                debug_assert!(child == below_left(hole.pos) || child == below_right(hole.pos));
                if hole.element() >= hole.get(child) {
                    // we are in order, exit
                    return;
                }
                hole.move_to(child);

                debug_assert_eq!(child, hole.pos);
                child = below_left(child);
            }
            // there is no right child, if we have a left child, try swapping it, otherwise we have reached the bottom
            if child < len && hole.element() < hole.get(child) {
                hole.move_to(child);
            }
        }
    }

    #[allow(unused)]
    fn sift_to_bottom_then_up(mut hole: Hole<K, P>) {
        let len = hole.data.len();
        unsafe {
            let mut child = below_left(hole.pos);
            while child < len - 1 {
                debug_assert_eq!(child, below_left(hole.pos));
                // we have both a left and a right child
                // select as child the one with the greatest priority
                child += (hole.get(child) <= hole.get(child + 1)) as usize;
                debug_assert!(child == below_left(hole.pos) || child == below_right(hole.pos));
                hole.move_to(child);

                debug_assert_eq!(child, hole.pos);
                child = below_left(child);
            }
            // there is no right child, if we have a left child, try swapping it, otherwise we have reached the bottom
            if child < len && hole.element() < hole.get(child) {
                hole.move_to(child);
            }
            Self::sift_hole_up(hole);
        }
    }
}

/// Hole represents a hole in a slice i.e., an index without valid value
/// (because it was moved from or duplicated).
/// In drop, `Hole` will restore the slice by filling the hole
/// position with the value that was originally removed.
struct Hole<'a, K: Copy + Into<usize>, P> {
    data: &'a mut [HeapEntry<K, P>],
    index: &'a mut [Option<Entry<P>>],
    elt: ManuallyDrop<HeapEntry<K, P>>,
    pos: usize,
}

#[allow(unused_unsafe)]
impl<'a, K: Copy + Into<usize>, P> Hole<'a, K, P> {
    /// Create a new `Hole` at index `pos`.
    ///
    /// Unsafe because pos must be within the data slice.
    #[inline]
    unsafe fn new(data: &'a mut [HeapEntry<K, P>], index: &'a mut [Option<Entry<P>>], pos: usize) -> Self {
        debug_assert!(pos < data.len());
        // SAFE: pos should be inside the slice
        let elt = unsafe { ptr::read(data.get_unchecked(pos)) };
        Hole {
            data,
            index,
            elt: ManuallyDrop::new(elt),
            pos,
        }
    }

    unsafe fn new_with_element(
        data: &'a mut [HeapEntry<K, P>],
        index: &'a mut [Option<Entry<P>>],
        pos: usize,
        elt: HeapEntry<K, P>,
    ) -> Self {
        let removed = unsafe { ptr::read(data.get_unchecked(pos)) };
        debug_assert!(matches!(index[removed.key.into()], Some(Out(_))));
        Hole {
            data,
            index,
            elt: ManuallyDrop::new(elt),
            pos,
        }
    }

    /// Returns a reference to the element removed.
    #[inline]
    fn element(&self) -> &HeapEntry<K, P> {
        &self.elt
    }

    /// Returns a reference to the element at `index`.
    ///
    /// Unsafe because index must be within the data slice and not equal to pos.
    #[inline]
    unsafe fn get(&self, index: usize) -> &HeapEntry<K, P> {
        debug_assert!(index != self.pos);
        debug_assert!(index < self.data.len());
        unsafe { self.data.get_unchecked(index) }
    }

    /// Move hole to new location
    ///
    /// Unsafe because index must be within the data slice and not equal to pos.
    #[inline]
    unsafe fn move_to(&mut self, index: usize) {
        debug_assert!(index != self.pos);
        debug_assert!(index < self.data.len());
        unsafe {
            let ptr = self.data.as_mut_ptr();
            let index_ptr: *const _ = ptr.add(index);
            let moved_key = (*index_ptr).key;
            let hole_ptr = ptr.add(self.pos);
            ptr::copy_nonoverlapping(index_ptr, hole_ptr, 1);
            let i = self.index.as_mut_ptr();
            let i = i.add(moved_key.into());
            ptr::write(i, Some(In(self.pos)));
        }
        self.pos = index;
    }
}

impl<K: Copy + Into<usize>, P> Drop for Hole<'_, K, P> {
    #[inline]
    fn drop(&mut self) {
        // fill the hole again
        unsafe {
            let pos = self.pos;
            let key = self.elt.key;
            ptr::copy_nonoverlapping(&*self.elt, self.data.get_unchecked_mut(pos), 1);
            // write the index with the final position of the moved element
            let i = self.index.as_mut_ptr();
            let i = i.add(key.into());
            ptr::write(i, Some(In(pos)));
        }
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

        let check_all_priorities = |heap: &IdxHeap<usize, f64>| {
            for (i, &priority) in priorities.iter().enumerate().take(N) {
                assert!(
                    eq(heap.priority(i), priority),
                    "Elt: {}, found: {}, expected: {}",
                    i,
                    heap.priority(i),
                    priority
                );
            }
        };

        let remove_n = |heap: &mut IdxHeap<usize, f64>, n: usize| -> Vec<usize> {
            let mut removed = Vec::new();
            let first = heap.pop().unwrap();
            removed.push(first);
            let mut previous_best = heap.priority(first);
            assert!(eq(previous_best, priorities[first]));
            println!("Removed: {first}");
            for _ in 1..n {
                let next = heap.pop().unwrap();
                let p = heap.priority(next);
                assert!(eq(p, priorities[next]));
                assert!(p <= previous_best, "{}", "p: {p}   prev:{previous_best}");
                previous_best = p;
                removed.push(next);
                println!("Removed: {next}");
                check_all_priorities(heap);
            }

            removed
        };
        let insert_all = |heap: &mut IdxHeap<usize, f64>, to_insert: &[usize]| {
            for elt in to_insert {
                heap.enqueue(*elt);
                println!("Inserted: {}, priority: {}", elt, priorities[*elt]);
                check_all_priorities(heap);
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
