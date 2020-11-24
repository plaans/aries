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
    pub fn set_backtrack_point(&mut self) {
        self.backtrack_points.push(self.events.len());
    }
    pub fn backtrack(&mut self) {
        if let Some(after_last) = self.backtrack_points.pop() {
            self.events.drain(after_last..);
            let bt_id = self.last_backtrack.as_ref().map_or(0, |bt| bt.id + 1);
            self.last_backtrack = Some(LastBacktrack {
                next_read: after_last,
                id: bt_id,
            });
        } else {
            panic!("No backtrack points left");
        }
    }
}

pub struct Q<V> {
    queue: Rc<RefCell<QInner<V>>>,
}

impl<V> Q<V> {
    pub fn new() -> Q<V> {
        Q {
            queue: Default::default(),
        }
    }

    pub fn writer(&self) -> QWriter<V> {
        QWriter {
            q: Q {
                queue: self.queue.clone(),
            },
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
}

pub struct QWriter<V> {
    q: Q<V>,
}
impl<V> QWriter<V> {
    /// Adds a single `value` to the queue.
    pub fn push(&mut self, value: V) {
        self.q.queue.borrow_mut().push(value)
    }

    /// Adds a sequence of `values` to the queue.
    pub fn append<Vs: IntoIterator<Item = V>>(&mut self, values: Vs) {
        self.q.queue.borrow_mut().append(values);
    }

    pub fn set_backtrack_point(&mut self) {
        self.q.queue.borrow_mut().set_backtrack_point();
    }

    pub fn backtrack(&mut self) {
        self.q.queue.borrow_mut().backtrack()
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
        let remaining = size - self.next_read;
        remaining
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
        let q = Q::new();
        q.writer().push(0);
        q.writer().push(1);
        let mut q2 = q.writer();
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

        q.writer().push(2);
        assert_eq!(r1.pop(), Some(2));
        assert_eq!(r2.pop(), Some(2));
        assert_eq!(r1.pop(), None);
        assert_eq!(r2.pop(), None);
    }

    #[test]
    fn test_backtracks() {
        let q = Q::new();
        let mut w = q.writer();

        w.push(1);
        w.push(2);
        w.set_backtrack_point();
        w.push(3);

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
        w.backtrack();
        assert_eq!(r1.pop(), None);
        assert_eq!(r2.pop(), None);
        assert_eq!(r3.pop(), Some(2));
        assert_eq!(r3.pop(), None);

        let mut r = q.reader();
        assert_eq!(r.pop(), Some(1));
        assert_eq!(r.pop(), Some(2));
        assert_eq!(r.pop(), None);

        w.set_backtrack_point();
        w.push(4);
        w.backtrack();
        w.push(5);
        w.set_backtrack_point();
        w.push(6);
        w.backtrack();
        assert_eq!(r.pop(), Some(5));
        assert_eq!(r.pop(), None);
    }
}
