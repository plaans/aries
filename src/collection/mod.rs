pub mod index_map;

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
    fn next(self) -> Self;
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
