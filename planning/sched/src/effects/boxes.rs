use std::collections::BTreeMap;

use aries::core::IntCst;
use itertools::Itertools;

/// A segment with a first and last elements
#[derive(Copy, Clone)]
pub struct Segment {
    pub first: IntCst,
    pub last: IntCst,
}

impl Segment {
    pub fn new(first: IntCst, last: IntCst) -> Self {
        Segment { first, last }
    }

    /// Returns `true` iff two segments overlap.
    ///
    pub fn overlaps(&self, other: &Segment) -> bool {
        !(self.last < other.first || other.last < self.first)
    }
}

/// An axis-aligned box, defined by its projection on all dimensions.
#[derive(Copy, Clone)]
pub struct Box<'a> {
    dimensions: &'a [Segment],
}

impl<'a> Box<'a> {
    pub fn new(dimensions: &'a [Segment]) -> Self {
        Self { dimensions }
    }

    /// returns true iff the two boxes overlap.
    ///
    /// Panics the boxes have different dimensions.
    pub fn overlaps(&self, other: Box<'a>) -> bool {
        self.dimensions
            .iter()
            .zip_eq(other.dimensions.iter())
            .all(|(a, b)| a.overlaps(b))
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

    pub fn boxes_from<'a>(&'a self, first_box: usize) -> impl Iterator<Item = Box<'a>> {
        self.segments[(first_box * self.num_dimensions)..]
            .chunks(self.num_dimensions)
            .map(|chunk| Box { dimensions: chunk })
    }
    pub fn tags_from(&self, first_box: usize) -> impl Iterator<Item = &Tag> + '_ {
        self.tags[first_box..].iter()
    }

    pub fn tagged_boxes<'a>(&'a self) -> impl Iterator<Item = (&'a Tag, Box<'a>)> {
        self.tagged_boxes_from(0)
    }
    pub fn tagged_boxes_from<'a>(&'a self, first_box: usize) -> impl Iterator<Item = (&'a Tag, Box<'a>)> {
        self.tags_from(first_box).zip_eq(self.boxes_from(first_box))
    }

    pub fn overlapping_boxes(&self) -> impl Iterator<Item = (&Tag, &Tag)> + '_ {
        self.tagged_boxes()
            .enumerate()
            .flat_map(|(i, tb1)| self.tagged_boxes_from(i + 1).map(move |tb2| (tb1, tb2)))
            .filter_map(|((t1, b1), (t2, b2))| if b1.overlaps(b2) { Some((t1, t2)) } else { None })
    }

    pub fn find_overlapping_with<'a>(&'a self, bx: Vec<Segment>) -> impl Iterator<Item = &'a Tag> {
        self.tagged_boxes()
            .filter_map(move |(t, b)| if Box::new(&bx).overlaps(b) { Some(t) } else { None })
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

    pub fn find_overlapping_with<'a>(&'a self, world: &World, bx: Vec<Segment>) -> impl Iterator<Item = &'a Tag> {
        self.worlds
            .get(world)
            .into_iter()
            .flat_map(move |w| w.find_overlapping_with(bx.clone()))
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
