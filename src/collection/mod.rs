pub mod heap;
pub mod index_map;
pub mod id_map;

pub struct Range<A> {
    first: A,
    last: A,
}
impl<A> Range<A> {
    pub fn new(first: A, last: A) -> Self {
        Range { first, last }
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
        let end = start.next_n(n - 1);
        Range::new(start, end)
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

        if prev <= self.last {
            Some(prev)
        } else {
            None
        }
    }
}
