pub mod heap;
pub mod id_map;
pub mod index_map;

pub struct Range<A> {
    first: A,
    after_last: A,
}
impl<A: Next> Range<A> {
    pub fn new(first: A, last: A) -> Self {
        Range {
            first,
            after_last: last.next(),
        }
    }
}

pub trait Next {
    fn next(self) -> Self
    where
        Self: Sized,
    {
        self.next_n(1)
    }
    fn next_n(self, n: usize) -> Self;

    fn first(n: usize) -> Range<Self>
    where
        Self: Sized + MinVal + Copy,
    {
        let start = Self::min_value();
        Range {
            first: start,
            after_last: start.next_n(n),
        }
    }
}

pub trait MinVal {
    fn min_value() -> Self;
}

impl<A: Next + Copy + PartialOrd> Iterator for Range<A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        let prev = self.first;
        self.first = prev.next();

        if prev < self.after_last {
            Some(prev)
        } else {
            None
        }
    }
}
