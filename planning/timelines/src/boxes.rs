use std::{collections::BTreeMap, fmt::Debug, ops::RangeInclusive};

use aries_solver::core::IntCst;
use enumeration::StreamingIterator;
use itertools::Itertools;
use smallvec::SmallVec;

/// A segment with a first and last elements
#[derive(Copy, Clone)]
pub struct Segment {
    pub first: IntCst,
    pub last: IntCst,
}
impl Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.first, self.last)
    }
}

impl Segment {
    pub fn new(first: IntCst, last: IntCst) -> Self {
        Segment { first, last }
    }

    /// A segment that spans all possible values
    pub fn all() -> Self {
        Self {
            first: IntCst::MIN,
            last: IntCst::MAX,
        }
    }

    /// A segment that contains conly a specific value
    pub fn point(val: IntCst) -> Self {
        Self { first: val, last: val }
    }

    /// A segment with no values.
    pub fn empty() -> Self {
        Self {
            first: IntCst::MAX,
            last: IntCst::MIN,
        }
    }

    /// Returns `true` iff two segments overlap.
    ///
    pub fn overlaps(&self, other: &Segment) -> bool {
        !(self.last < other.first || other.last < self.first)
    }

    pub fn union(&mut self, other: &Segment) {
        self.first = self.first.min(other.first);
        self.last = self.last.max(other.last);
    }

    pub fn points(&self) -> RangeInclusive<IntCst> {
        self.first..=self.last
    }
}

impl From<(IntCst, IntCst)> for Segment {
    fn from((lb, ub): (IntCst, IntCst)) -> Self {
        Self::new(lb, ub)
    }
}

pub(crate) type Segments = SmallVec<[Segment; 5]>;

#[derive(Clone, Debug)]
pub struct BBox {
    dimensions: Segments,
}
impl BBox {
    pub fn new(dims: impl Into<Segments>) -> Self {
        Self {
            dimensions: dims.into(),
        }
    }

    pub fn union(&mut self, other: BoxRef<'_>) {
        self.dimensions
            .iter_mut()
            .zip_eq(other.dimensions.iter())
            .for_each(|(s, o)| s.union(o));
    }

    pub fn as_ref<'a>(&'a self) -> BoxRef<'a> {
        BoxRef::new(&self.dimensions)
    }

    pub fn segments(&self) -> &[Segment] {
        &self.dimensions
    }
}
/// An axis-aligned box, defined by its projection on all dimensions.
#[derive(Copy, Clone)]
pub struct BoxRef<'a> {
    dimensions: &'a [Segment],
}

impl<'a> BoxRef<'a> {
    pub fn new(dimensions: &'a [Segment]) -> Self {
        Self { dimensions }
    }

    /// returns true iff the two boxes overlap.
    ///
    /// Panics the boxes have different dimensions.
    pub fn overlaps(&self, other: BoxRef<'a>) -> bool {
        self.dimensions
            .iter()
            .zip_eq(other.dimensions.iter())
            .all(|(a, b)| a.overlaps(b))
    }

    pub fn to_owned(&self) -> BBox {
        BBox::new(self.dimensions)
    }

    pub fn last(&self) -> Option<Segment> {
        self.dimensions.last().copied()
    }

    pub fn drop_head(&self, n: usize) -> Self {
        Self {
            dimensions: &self.dimensions[n..],
        }
    }
    pub fn drop_tail(&self, n: usize) -> Self {
        Self {
            dimensions: &self.dimensions[..(self.dimensions.iter().len() - n)],
        }
    }

    pub fn points(self) -> impl StreamingIterator<Item = [IntCst]> {
        let generators = self.dimensions.iter().map(|seg| seg.points()).collect_vec();
        enumeration::enumerate(generators)
    }
}

/// A set of homoneous tagged boxes (all of same dimension) each with a particular tag.
#[derive(Clone)]
pub struct BoxWorld<Tag> {
    segments: Vec<Segment>,
    tags: Vec<Tag>,
    num_dimensions: usize,
}

impl<Tag> BoxWorld<Tag> {
    pub fn new(dimensions: usize) -> Self {
        Self {
            segments: Vec::with_capacity(128 * dimensions),
            tags: Vec::with_capacity(128),
            num_dimensions: dimensions,
        }
    }

    /// Adds a box (defined by its projection on all dimensions.)
    pub fn add(&mut self, bx: &[Segment], tag: Tag) {
        assert_eq!(bx.len(), self.num_dimensions);
        self.segments.extend_from_slice(bx);
        self.tags.push(tag);
    }

    pub fn boxes_from<'a>(&'a self, first_box: usize) -> impl Iterator<Item = BoxRef<'a>> {
        self.segments[(first_box * self.num_dimensions)..]
            .chunks(self.num_dimensions)
            .map(|chunk| BoxRef { dimensions: chunk })
    }
    pub fn tags_from(&self, first_box: usize) -> impl Iterator<Item = &Tag> + '_ {
        self.tags[first_box..].iter()
    }

    pub fn tagged_boxes<'a>(&'a self) -> impl Iterator<Item = (&'a Tag, BoxRef<'a>)> {
        self.tagged_boxes_from(0)
    }
    pub fn tagged_boxes_from<'a>(&'a self, first_box: usize) -> impl Iterator<Item = (&'a Tag, BoxRef<'a>)> {
        self.tags_from(first_box).zip_eq(self.boxes_from(first_box))
    }

    pub fn overlapping_boxes(&self) -> impl Iterator<Item = (&Tag, &Tag)> + '_ {
        self.tagged_boxes()
            .enumerate()
            .flat_map(|(i, tb1)| self.tagged_boxes_from(i + 1).map(move |tb2| (tb1, tb2)))
            .filter_map(|((t1, b1), (t2, b2))| if b1.overlaps(b2) { Some((t1, t2)) } else { None })
    }

    pub fn find_overlapping_with<'a>(&'a self, bx: BoxRef<'a>) -> impl Iterator<Item = &'a Tag> {
        self.tagged_boxes()
            .filter_map(move |(t, b)| if bx.overlaps(b) { Some(t) } else { None })
    }
}

/// A set of tagged boxes, partitioned into worlds.
///
/// This provides fairly efficient ways to find the collision between any pair of boxes of a unique world.
#[derive(Clone)]
pub struct BoxUniverse<World, Tag> {
    worlds: BTreeMap<World, BoxWorld<Tag>>,
}

impl<World: Ord + Clone, Tag> BoxUniverse<World, Tag> {
    pub fn new() -> Self {
        Self {
            worlds: Default::default(),
        }
    }

    /// Adds a new box to the given world. The box is associated to a Tag that will provided when checking for overlaps.
    pub fn add_box(&mut self, world: &World, bx: &[Segment], tag: Tag) {
        self.worlds
            .entry(world.clone())
            .or_insert_with(|| BoxWorld::new(bx.len()))
            .add(bx, tag);
    }

    /// Returns the tags of all pairs of overlapping bowes in the universe. Note that two boxes are not considered as overlapping if they are not in the same world.
    pub fn overlapping_boxes(&self) -> impl Iterator<Item = (&Tag, &Tag)> + '_ {
        self.worlds.values().flat_map(|world| world.overlapping_boxes())
    }

    pub fn find_overlapping_with<'a>(&'a self, world: &World, bx: BoxRef<'a>) -> impl Iterator<Item = &'a Tag> {
        self.worlds
            .get(world)
            .into_iter()
            .flat_map(move |w| w.find_overlapping_with(bx))
    }

    pub fn has_world(&self, world: &World) -> bool {
        self.worlds.contains_key(world)
    }
}

impl<World: Ord + Clone, Tag> Default for BoxUniverse<World, Tag> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_segments_overlap() {
        assert!(Segment::new(1, 4).overlaps(&Segment::new(2, 5)));
        assert!(Segment::new(1, 4).overlaps(&Segment::new(2, 3)));
        assert!(Segment::new(1, 4).overlaps(&Segment::new(4, 100)));
        assert!(!Segment::new(1, 4).overlaps(&Segment::new(5, 7)));
        assert!(!Segment::new(1, 4).overlaps(&Segment::new(-1, 0)));
    }
}

pub mod enumeration {
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
        generators: Vec<Iterable>,
        cur: Vec<Iterable>,
        sol: Vec<Item>,
        is_first: bool,
        finished: bool,
    }

    impl<Item, Iterable: Iterator<Item = Item> + Clone> Combination<Item, Iterable> {
        pub fn new(instances: Vec<Iterable>) -> Self {
            let size = instances.len();
            Combination {
                generators: instances.clone(),
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
            } else if self.is_first && self.generators.is_empty() {
                // empty generator, we should only generate the unit result : []
                self.is_first = false;
                return;
            } else if !self.is_first {
                if self.sol.is_empty() {
                    self.finished = true;
                    return;
                }
                debug_assert!(self.sol.len() == self.generators.len());
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
                    self.cur[lvl] = self.generators[lvl].clone();
                }
                if self.sol.len() == self.generators.len() {
                    return; // solution remaining
                }
            }
        }

        fn get(&self) -> Option<&Self::Item> {
            if self.finished {
                None
            } else {
                debug_assert_eq!(self.sol.len(), self.generators.len());
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

            let xs = ["x1", "x2"];
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
}
