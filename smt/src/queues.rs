use std::borrow::Borrow;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Q<V> {
    queue: Rc<RefCell<Vec<V>>>,
}

impl<V> Q<V> {
    pub fn new() -> Q<V> {
        Q {
            queue: Default::default(),
        }
    }

    fn push(&mut self, value: V) {
        self.queue.borrow_mut().push(value)
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
        }
    }
}

pub struct QWriter<V> {
    q: Q<V>,
}
impl<V> QWriter<V> {
    pub fn push(&mut self, value: V) {
        self.q.push(value)
    }
}

pub struct QReader<V> {
    q: Q<V>,
    next_read: usize,
}

impl<V> QReader<V> {
    pub fn pop(&mut self) -> Option<V>
    where
        V: Clone,
    {
        let queue: &RefCell<_> = self.q.queue.borrow();
        let queue = queue.borrow();
        let next = self.next_read;
        if next < queue.len() {
            self.next_read += 1;
            Some(queue[next].clone())
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

    fn size_hint(&self) -> (usize, Option<usize>) {
        let queue: &RefCell<_> = self.q.queue.borrow();
        let size = queue.borrow().len();
        let remaining = size - self.next_read;
        (remaining, Some(remaining))
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
        let mut q2 = q.writer();
        q2.push(5);

        let mut r1 = q.reader();
        for i in &mut r1 {
            println!("R1: {}", i);
        }
        let mut r2 = q.reader();
        for i in &mut r2 {
            println!("R2: {}", i);
        }

        q.push(2);
        for i in &mut r1 {
            println!("R1: {}", i);
        }
    }
}
