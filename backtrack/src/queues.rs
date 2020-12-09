use crate::{Backtrack, BacktrackWith};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::rc::Rc;

struct LastBacktrack {
    next_read: usize,
    id: u64,
}

struct QInner<V> {
    events: Vec<V>,
    backtrack_points: Vec<usize>,
    last_backtrack: Option<LastBacktrack>,
}
impl<V> Default for QInner<V> {
    fn default() -> Self {
        QInner {
            events: Default::default(),
            backtrack_points: Default::default(),
            last_backtrack: None,
        }
    }
}
impl<V> QInner<V> {
    pub fn push(&mut self, value: V) {
        self.events.push(value);
    }
    pub fn append<Vs: IntoIterator<Item = V>>(&mut self, values: Vs) {
        self.events.extend(values);
    }

    fn backtrack_with_callback(&mut self, mut f: impl FnMut(V)) {
        let after_last = self.backtrack_points.pop().expect("No backup points left.");
        while after_last < self.events.len() {
            let ev = self.events.pop().expect("No events left");
            f(ev)
        }
        let bt_id = self.last_backtrack.as_ref().map_or(0, |bt| bt.id + 1);
        self.last_backtrack = Some(LastBacktrack {
            next_read: after_last,
            id: bt_id,
        });
    }
}

impl<V> Backtrack for QInner<V> {
    fn save_state(&mut self) -> u32 {
        self.backtrack_points.push(self.events.len());
        self.num_saved() - 1
    }

    fn num_saved(&self) -> u32 {
        self.backtrack_points.len() as u32
    }

    fn restore_last(&mut self) {
        self.backtrack_with_callback(|_| ())
    }
}

impl<V> BacktrackWith for QInner<V> {
    type Event = V;
    fn restore_last_with<F: FnMut(Self::Event)>(&mut self, callback: F) {
        self.backtrack_with_callback(callback)
    }
}

#[derive(Clone)]
pub struct Q<V> {
    queue: Rc<RefCell<QInner<V>>>,
}
impl<V> Default for Q<V> {
    fn default() -> Self {
        Q {
            queue: Default::default(),
        }
    }
}

impl<V> Q<V> {
    pub fn new() -> Q<V> {
        Q {
            queue: Default::default(),
        }
    }

    pub fn reader(&self) -> QReader<V> {
        QReader {
            q: Q {
                queue: self.queue.clone(),
            },
            next_read: 0,
            last_backtrack: None,
        }
    }

    /// Adds a single `value` to the queue.
    pub fn push(&mut self, value: V) {
        self.queue.borrow_mut().push(value)
    }

    /// Adds a sequence of `values` to the queue.
    pub fn append<Vs: IntoIterator<Item = V>>(&mut self, values: Vs) {
        self.queue.borrow_mut().append(values);
    }
}

impl<V> Backtrack for Q<V> {
    fn save_state(&mut self) -> u32 {
        self.queue.borrow_mut().save_state()
    }

    fn num_saved(&self) -> u32 {
        let x: &RefCell<_> = self.queue.borrow();
        x.borrow().num_saved()
    }

    fn restore_last(&mut self) {
        self.queue.borrow_mut().restore_last();
    }
}

impl<V> BacktrackWith for Q<V> {
    type Event = V;

    fn restore_last_with<F: FnMut(Self::Event)>(&mut self, callback: F) {
        self.queue.borrow_mut().restore_last_with(callback);
    }
}

pub struct QReader<V> {
    q: Q<V>,
    next_read: usize,
    last_backtrack: Option<u64>,
}

impl<V> QReader<V> {
    fn sync_backtrack(&mut self) {
        let queue: &RefCell<_> = self.q.queue.borrow();
        let queue = queue.borrow();
        if let Some(x) = &queue.last_backtrack {
            // a backtrack has already happened in the queue, check if we are in sync
            if self.last_backtrack != Some(x.id) {
                // we have not handled this backtrack, backtrack now if have have read some
                // cancelled output
                if self.next_read > x.next_read {
                    self.next_read = x.next_read;
                }
                self.last_backtrack = Some(x.id);
            }
        }
    }

    pub fn len(&mut self) -> usize {
        self.sync_backtrack();
        let queue: &RefCell<_> = self.q.queue.borrow();
        let size = queue.borrow().events.len();
        size - self.next_read
    }

    pub fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    pub fn pop(&mut self) -> Option<V>
    where
        V: Clone,
    {
        self.sync_backtrack();
        let queue: &RefCell<_> = self.q.queue.borrow();
        let queue = queue.borrow();

        let next = self.next_read;
        if next < queue.events.len() {
            self.next_read += 1;
            Some(queue.events[next].clone())
        } else {
            None
        }
    }
}

impl<V: Clone> Iterator for QReader<V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queues() {
        let mut q = Q::new();
        q.push(0);
        q.push(1);
        let mut q2 = q.clone();
        q2.push(5);

        let mut r1 = q.reader();
        assert_eq!(r1.pop(), Some(0));
        assert_eq!(r1.pop(), Some(1));
        assert_eq!(r1.pop(), Some(5));
        assert_eq!(r1.pop(), None);

        let mut r2 = q.reader();
        assert_eq!(r2.pop(), Some(0));
        assert_eq!(r2.pop(), Some(1));
        assert_eq!(r2.pop(), Some(5));
        assert_eq!(r2.pop(), None);

        q.push(2);
        assert_eq!(r1.pop(), Some(2));
        assert_eq!(r2.pop(), Some(2));
        assert_eq!(r1.pop(), None);
        assert_eq!(r2.pop(), None);
    }

    #[test]
    fn test_backtracks() {
        let mut q = Q::new();

        q.push(1);
        q.push(2);
        q.save_state();
        q.push(3);

        let mut r = q.reader();
        assert_eq!(r.pop(), Some(1));
        assert_eq!(r.pop(), Some(2));
        assert_eq!(r.pop(), Some(3));

        let mut r1 = q.reader();
        let mut r2 = q.reader();
        let mut r3 = q.reader();
        assert_eq!(r1.pop(), Some(1));
        assert_eq!(r1.pop(), Some(2));
        assert_eq!(r1.pop(), Some(3));
        assert_eq!(r2.pop(), Some(1));
        assert_eq!(r2.pop(), Some(2));
        assert_eq!(r3.pop(), Some(1));
        q.restore_last();
        assert_eq!(r1.pop(), None);
        assert_eq!(r2.pop(), None);
        assert_eq!(r3.pop(), Some(2));
        assert_eq!(r3.pop(), None);

        let mut r = q.reader();
        assert_eq!(r.pop(), Some(1));
        assert_eq!(r.pop(), Some(2));
        assert_eq!(r.pop(), None);

        q.save_state();
        q.push(4);
        q.restore_last();
        q.push(5);
        q.save_state();
        q.push(6);
        q.restore_last();
        assert_eq!(r.pop(), Some(5));
        assert_eq!(r.pop(), None);
    }
}
