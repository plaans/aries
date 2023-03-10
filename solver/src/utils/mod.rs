pub(crate) mod cpu_time;
pub mod input;

use std::fmt::{Display, Error, Formatter};

/// A custom type to extract the formatter and feed it to formal_impl
/// Source: `<https://github.com/rust-lang/rust/issues/46591#issuecomment-350437057>`
pub struct Fmt<F>(pub F)
where
    F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result;

impl<F> std::fmt::Display for Fmt<F>
where
    F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        (self.0)(f)
    }
}

#[allow(clippy::while_let_on_iterator)]
pub fn disp_iter<T: Display>(f: &mut Formatter<'_>, iterable: &[T], sep: &str) -> Result<(), Error> {
    let mut i = iterable.iter();
    if let Some(first) = i.next() {
        write!(f, "{first}")?;
        while let Some(other) = i.next() {
            write!(f, "{sep}")?;
            write!(f, "{other}")?;
        }
    }
    Result::Ok(())
}

pub use streaming_iterator::StreamingIterator;

/// Enumerate all possible combinations that can be gathered from a vector of iterators.
///
/// It expects a vector of iterators vec![xs, ys, zs, ...].
///
/// The result is a streaming operator that whose element is a slice of values [x, y, z, ...]
/// where each value is pick from the corresponding iterator (e.g x from xs, y from ys).
/// As each iterator is potentially iterated over multiple times, they must be cloneable.
///
/// The call to `enumerate(vec![0..2, 5..7])` will result in the four following combinations
/// [0, 5]
/// [0, 6]
/// [1, 5]
/// [1, 6]
pub fn enumerate<Item, Iter: Iterator<Item = Item> + Clone>(
    generators: Vec<Iter>,
) -> impl StreamingIterator<Item = [Item]> {
    Combination::new(generators)
}

struct Combination<Item, Iterable> {
    gen: Vec<Iterable>,
    cur: Vec<Iterable>,
    sol: Vec<Item>,
    is_first: bool,
    finished: bool,
}

impl<Item, Iterable: Iterator<Item = Item> + Clone> Combination<Item, Iterable> {
    pub fn new(instances: Vec<Iterable>) -> Self {
        let size = instances.len();
        Combination {
            gen: instances.clone(),
            cur: instances,
            sol: Vec::with_capacity(size),
            is_first: true,
            finished: false,
        }
    }
}

impl<I, It: Iterator<Item = I> + Clone> streaming_iterator::StreamingIterator for Combination<I, It> {
    type Item = [I];

    fn advance(&mut self) {
        if self.finished {
            return;
        } else if self.is_first && self.gen.is_empty() {
            // empty generator, we should only generate the unit result : []
            self.is_first = false;
            return;
        } else if !self.is_first {
            if self.sol.is_empty() {
                self.finished = true;
                return;
            }
            debug_assert!(self.sol.len() == self.gen.len());
            self.sol.pop();
        }
        self.is_first = false;
        loop {
            let lvl = self.sol.len();
            if let Some(i) = self.cur[lvl].next() {
                self.sol.push(i);
            } else {
                if self.sol.is_empty() {
                    self.finished = true;
                    return;
                }
                self.sol.pop();
                self.cur[lvl] = self.gen[lvl].clone();
            }
            if self.sol.len() == self.gen.len() {
                return; // solution remaining
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        if self.finished {
            None
        } else {
            debug_assert_eq!(self.sol.len(), self.gen.len());
            Some(self.sol.as_slice())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    #[test]
    fn generation() {
        let gens = vec![0..2, 1..3];

        let mut generated: Vec<Vec<i32>> = Vec::new();
        let mut iter = enumerate(gens.clone());
        while let Some(x) = iter.next() {
            generated.push(x.to_vec());
        }
        assert_eq!(generated, vec![vec![0, 1], vec![0, 2], vec![1, 1], vec![1, 2]]);

        let mut iter = enumerate(gens);
        while let Some(x) = iter.next() {
            println!("{x:?}");
        }

        let xs = vec!["x1", "x2"];
        let it = enumerate(vec![xs.iter()]);
        assert_eq!(it.count(), 2);

        assert_eq!(enumerate(Vec::<Range<i32>>::new()).count(), 1);
        assert_eq!(enumerate(vec![1..2, 1..2, 1..2, 1..2]).count(), 1);
        assert_eq!(enumerate(vec![1..3, 1..2, 1..2, 1..2]).count(), 2);
        assert_eq!(enumerate(vec![1..3, 1..3, 1..2, 1..2]).count(), 4);
        assert_eq!(enumerate(vec![1..3, 1..3, 1..3, 1..2]).count(), 8);
        assert_eq!(enumerate(vec![1..3, 1..3, 1..3, 1..3]).count(), 16);
        assert_eq!(enumerate(vec![1..2, 1..3, 1..3, 1..3]).count(), 8);
        assert_eq!(enumerate(vec![1..1, 1..3, 1..3, 1..3]).count(), 0);
        assert_eq!(enumerate(vec![1..3, 1..1, 1..3, 1..3]).count(), 0);
        assert_eq!(enumerate(vec![1..3, 1..3, 1..1, 1..3]).count(), 0);
        assert_eq!(enumerate(vec![1..3, 1..3, 1..3, 1..1]).count(), 0);
    }
}
