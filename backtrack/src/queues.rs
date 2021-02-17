use crate::{Backtrack, BacktrackWith};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::num::NonZeroU32;

/// Represents a decision level.
///
/// The ROOT is the level at which no decision has been made.
/// Each time a decision is made, the decision level increases.
///
/// As a layout optimization, the internal representation disallows the 0 value.
/// This enables the compiler to use this value to reprensent an Option<DecLvl>
/// on 32 bits (rather than 64 without this optimisation).
/// This niche is especially useful for representing an Option<TrailLoc>.
#[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub struct DecLvl(NonZeroU32);

impl DecLvl {
    /// Represents the root decision level, at which no decision has been taken yet.
    pub const ROOT: DecLvl = Self::new(0);

    pub const fn new(num_saved: u32) -> Self {
        unsafe { DecLvl(NonZeroU32::new_unchecked(num_saved + 1)) }
    }

    /// Returns an integer representation of the decision level.
    /// O represents the ROOT.
    pub fn to_int(self) -> u32 {
        self.0.get() - 1
    }
}

impl std::ops::Add<i32> for DecLvl {
    type Output = DecLvl;

    #[inline]
    fn add(self, rhs: i32) -> Self::Output {
        Self::new(((self.to_int() as i32) + rhs) as u32)
    }
}
impl std::ops::AddAssign<i32> for DecLvl {
    fn add_assign(&mut self, rhs: i32) {
        *self = *self + rhs
    }
}
impl std::ops::Sub<i32> for DecLvl {
    type Output = DecLvl;

    /// Decreases the decision level by the given amount.
    ///
    /// ```
    /// use aries_backtrack::DecLvl;
    /// let a = DecLvl::ROOT +1;
    /// let b = DecLvl::ROOT +9;
    /// //assert_ne!(a, b);
    /// //assert_ne!(a, DecLvl::ROOT);
    /// let c = b - 8;
    /// assert_eq!(c, a);
    ///
    /// ```
    #[inline]
    fn sub(self, rhs: i32) -> Self::Output {
        self + (-rhs)
    }
}
impl std::ops::SubAssign<i32> for DecLvl {
    fn sub_assign(&mut self, rhs: i32) {
        *self = *self - rhs
    }
}

impl From<u32> for DecLvl {
    fn from(u: u32) -> Self {
        DecLvl::new(u)
    }
}
impl From<usize> for DecLvl {
    fn from(u: usize) -> Self {
        DecLvl::new(u as u32)
    }
}
impl From<DecLvl> for usize {
    fn from(dl: DecLvl) -> Self {
        dl.to_int() as usize
    }
}

impl std::ops::Index<DecLvl> for Vec<EventIndex> {
    type Output = EventIndex;

    fn index(&self, index: DecLvl) -> &Self::Output {
        &self[usize::from(index) - 1]
    }
}

impl std::fmt::Debug for DecLvl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dl({})", self.to_int())
    }
}

#[derive(Copy, Clone)]
struct LastBacktrack {
    next_read: usize,
    id: u64,
}

pub type EventIndex = u32;

#[derive(Clone)]
pub struct ObsTrail<V> {
    events: Vec<V>,
    /// Maps each decision level [DecLvl] with the index of its first event.
    backtrack_points: Vec<EventIndex>,
    last_backtrack: Option<LastBacktrack>,
}
impl<V> Default for ObsTrail<V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<V> ObsTrail<V> {
    pub fn new() -> Self {
        ObsTrail {
            events: Default::default(),
            backtrack_points: Default::default(),
            last_backtrack: None,
        }
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn next_slot(&self) -> TrailLoc {
        TrailLoc {
            decision_level: self.current_decision_level(),
            event_index: self.len() as EventIndex,
        }
    }

    pub fn push(&mut self, value: V) {
        self.events.push(value);
    }
    pub fn pop(&mut self) -> Option<V> {
        self.events.pop()
    }
    pub fn peek(&self) -> Option<&V> {
        self.events.last()
    }
    pub fn append<Vs: IntoIterator<Item = V>>(&mut self, values: Vs) {
        self.events.extend(values);
    }

    /// Creates a new reader for this queue
    pub fn reader(&self) -> ObsTrailCursor<V> {
        ObsTrailCursor {
            next_read: 0,
            last_backtrack: None,
            _phantom: Default::default(),
        }
    }

    fn backtrack_with_callback(&mut self, mut f: impl FnMut(&V)) {
        let after_last = self.backtrack_points.pop().expect("No backup points left.") as usize;
        let to_undo = &self.events[after_last..];
        for ev in to_undo.iter().rev() {
            f(ev)
        }
        self.events.drain(after_last..);
        let bt_id = self.last_backtrack.as_ref().map_or(0, |bt| bt.id + 1);
        self.last_backtrack = Some(LastBacktrack {
            next_read: after_last,
            id: bt_id,
        });
    }

    pub fn num_events(&self) -> u32 {
        self.len() as u32
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn current_decision_level(&self) -> DecLvl {
        DecLvl::new(self.backtrack_points.len() as u32)
    }

    pub fn get_event(&self, id: EventIndex) -> &V {
        &self.events[id as usize]
    }

    /// Returns a slice of all events, in chronological order.
    pub fn events(&self) -> &[V] {
        &self.events
    }

    /// Looks up the last event matching the predicate `pred`.
    /// Search goes backward in the list of event and stops when either
    ///  - no event remains
    ///  - the predicate keep_going(decision_level, event_index) returns true, where
    ///    - `decision_level` is the current decision level (going from the current one down to 0)
    ///    - `event_index` is the current event index (going from the index of the last event down to 0)
    ///
    /// # Usage
    /// ```
    /// use aries_backtrack::*;
    /// let mut q = ObsTrail::new();
    /// q.push(0); // decision_level: 0, index: 0
    /// q.push(1); // decision_level: 0, index: 1
    /// q.save_state();
    /// q.push(5);  // decision_level: 1, index: 2
    /// // look up all events for the last one that is lesser than or equal to 1
    /// let te = q.last_event_matching(|n| *n <= 1, |_, _| true).unwrap();
    /// assert_eq!(te.loc.decision_level, DecLvl::ROOT);
    /// assert_eq!(te.loc.event_index, 1);
    /// assert_eq!(*te.event, 1);
    /// // only lookup in the last decision level
    /// let te = q.last_event_matching(|n| *n <= 1, |dl, _| dl > DecLvl::ROOT);
    /// assert!(te.is_none());
    /// ```
    pub fn last_event_matching(
        &self,
        pred: impl Fn(&V) -> bool,
        keep_going: impl Fn(DecLvl, EventIndex) -> bool,
    ) -> Option<TrailEvent<V>>
    where
        V: Debug,
    {
        let mut decision_level = self.current_decision_level();

        println!("SEARCHING");
        self.print();

        for event_index in (0..self.events.len()).rev() {
            println!(
                "({:?}, {:?})   {:?}",
                decision_level,
                event_index,
                if decision_level > DecLvl::ROOT {
                    self.backtrack_points[decision_level]
                } else {
                    99999
                }
            );
            // let event_index = event_index as EventIndex;
            if !keep_going(decision_level, event_index as EventIndex) {
                return None;
            }
            let e = &self.events[event_index];
            if pred(e) {
                return Some(TrailEvent {
                    loc: TrailLoc {
                        decision_level,
                        event_index: event_index as u32,
                    },
                    event: &self.events[event_index],
                });
            }

            if decision_level > DecLvl::ROOT && self.backtrack_points[decision_level] == event_index as EventIndex {
                println!("  before: {:?}", decision_level);
                decision_level -= 1;
                println!("  after: {:?}", decision_level);
            }
        }
        None
    }

    /// Prints the content of the trail to standard output, specifying the decision levels.
    pub fn print(&self)
    where
        V: std::fmt::Debug,
    {
        let mut dl = 0;
        for i in 0..self.num_events() {
            print!("id: {:<4} ", i);
            if dl < self.backtrack_points.len() && self.backtrack_points[dl] == i {
                dl += 1;
                print!("dl: {:<4} ", dl);
            } else {
                print!("         ");
            }
            println!("{:?}", self.events[i as usize]);
        }
    }
}

impl<V> Backtrack for ObsTrail<V> {
    fn save_state(&mut self) -> DecLvl {
        self.backtrack_points.push(self.events.len() as EventIndex);
        self.current_decision_level()
    }

    fn num_saved(&self) -> u32 {
        self.backtrack_points.len() as u32
    }

    fn restore_last(&mut self) {
        self.backtrack_with_callback(|_| ())
    }
}

impl<V> BacktrackWith for ObsTrail<V> {
    type Event = V;
    fn restore_last_with<F: FnMut(&Self::Event)>(&mut self, callback: F) {
        self.backtrack_with_callback(callback)
    }
}

#[derive(Copy, Clone)]
pub struct TrailLoc {
    /// Decision level at which an event is located
    pub decision_level: DecLvl,
    /// Index of an event in the event list. Also represents the number of events that occurred before it
    pub event_index: u32,
}

impl PartialEq for TrailLoc {
    fn eq(&self, other: &Self) -> bool {
        self.event_index == other.event_index
    }
}
impl Eq for TrailLoc {}
impl Ord for TrailLoc {
    fn cmp(&self, other: &Self) -> Ordering {
        self.event_index.cmp(&other.event_index)
    }
}
impl PartialOrd for TrailLoc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for TrailLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TrailLoc(dl={}, id={}",
            self.decision_level.to_int(),
            self.event_index
        )
    }
}

/// Represents an event and its position in a trail
pub struct TrailEvent<'a, V> {
    /// location of the event in the trail
    pub loc: TrailLoc,
    /// An event in the trail.
    /// It is a reference, that links to the queue.
    pub event: &'a V,
}

#[derive(Clone)]
pub struct ObsTrailCursor<V> {
    next_read: usize,
    last_backtrack: Option<u64>,
    _phantom: PhantomData<V>,
}
impl<V> ObsTrailCursor<V> {
    /// Create a new cursor that is not bound to any queue.
    /// The cursor should only read from a single queue. This is enforced in debug mode
    /// by recording the ID of the read queue on the first read and checking that read is made
    /// on a queue with the same id on all subsequent reads.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ObsTrailCursor {
            next_read: 0,
            last_backtrack: None,
            _phantom: Default::default(),
        }
    }

    // TODO: check correctness if more than one backtrack occurred between two synchronisations
    fn sync_backtrack(&mut self, queue: &ObsTrail<V>) {
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

    pub fn num_pending(&mut self, queue: &ObsTrail<V>) -> usize {
        self.sync_backtrack(queue);
        let size = queue.events.len();
        size - self.next_read
    }

    pub fn pop<'q>(&mut self, queue: &'q ObsTrail<V>) -> Option<&'q V> {
        self.sync_backtrack(queue);

        let next = self.next_read;
        if next < queue.events.len() {
            self.next_read += 1;
            Some(&queue.events[next])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queues() {
        let mut q = ObsTrail::new();
        q.push(0);
        q.push(1);

        q.push(5);

        let mut r1 = q.reader();
        assert_eq!(r1.pop(&q), Some(&0));
        assert_eq!(r1.pop(&q), Some(&1));
        assert_eq!(r1.pop(&q), Some(&5));
        assert_eq!(r1.pop(&q), None);

        let mut r2 = q.reader();
        assert_eq!(r2.pop(&q), Some(&0));
        assert_eq!(r2.pop(&q), Some(&1));
        assert_eq!(r2.pop(&q), Some(&5));
        assert_eq!(r2.pop(&q), None);

        q.push(2);
        assert_eq!(r1.pop(&q), Some(&2));
        assert_eq!(r2.pop(&q), Some(&2));
        assert_eq!(r1.pop(&q), None);
        assert_eq!(r2.pop(&q), None);
    }

    #[test]
    fn test_backtracks() {
        let mut q = ObsTrail::new();

        q.push(1);
        q.push(2);
        q.save_state();
        q.push(3);

        let mut r = q.reader();
        assert_eq!(r.pop(&q), Some(&1));
        assert_eq!(r.pop(&q), Some(&2));
        assert_eq!(r.pop(&q), Some(&3));

        let mut r1 = q.reader();
        let mut r2 = q.reader();
        let mut r3 = q.reader();
        assert_eq!(r1.pop(&q), Some(&1));
        assert_eq!(r1.pop(&q), Some(&2));
        assert_eq!(r1.pop(&q), Some(&3));
        assert_eq!(r2.pop(&q), Some(&1));
        assert_eq!(r2.pop(&q), Some(&2));
        assert_eq!(r3.pop(&q), Some(&1));
        q.restore_last();
        assert_eq!(r1.pop(&q), None);
        assert_eq!(r2.pop(&q), None);
        assert_eq!(r3.pop(&q), Some(&2));
        assert_eq!(r3.pop(&q), None);

        let mut r = q.reader();
        assert_eq!(r.pop(&q), Some(&1));
        assert_eq!(r.pop(&q), Some(&2));
        assert_eq!(r.pop(&q), None);

        q.save_state();
        q.push(4);
        q.restore_last();
        q.push(5);
        q.save_state();
        q.push(6);
        q.restore_last();
        assert_eq!(r.pop(&q), Some(&5));
        assert_eq!(r.pop(&q), None);
    }

    #[test]
    fn event_lookups() {
        let mut q = ObsTrail::new();

        q.push(1); // (0, 0)
        q.push(2); // (0, 1)
        q.save_state();
        q.push(3); // (1, 2)
        q.push(4); // (1, 3)
        q.save_state();
        q.push(5); // (2, 4)
        q.push(3); // (2, 5)

        let test_all = |n: i32, expected_pos: Option<(DecLvl, EventIndex)>| match q
            .last_event_matching(|ev| ev == &n, |_, _| true)
        {
            None => assert!(expected_pos.is_none()),
            Some(e) => {
                assert_eq!(Some((e.loc.decision_level, e.loc.event_index)), expected_pos);
                assert_eq!(*e.event, n);
            }
        };
        let dl = |i| DecLvl::new(i);
        test_all(99, None);
        test_all(-1, None);
        test_all(1, Some((dl(0), 0)));
        test_all(2, Some((dl(0), 1)));
        test_all(3, Some((dl(2), 5)));
        test_all(4, Some((dl(1), 3)));
        test_all(5, Some((dl(2), 4)));

        // finds the position of the event, restricting itself to the last decision level
        let test_last = |n: i32, expected_pos: Option<(DecLvl, EventIndex)>| {
            let last_decision_level = q.current_decision_level();
            match q.last_event_matching(|ev| ev == &n, |dl, _| dl >= last_decision_level) {
                None => assert!(expected_pos.is_none()),
                Some(e) => {
                    assert_eq!(Some((e.loc.decision_level, e.loc.event_index)), expected_pos);
                    assert_eq!(*e.event, n);
                }
            };
        };

        test_last(99, None);
        test_last(-1, None);
        test_last(1, None);
        test_last(2, None);
        test_last(3, Some((dl(2), 5)));
        test_last(4, None);
        test_last(5, Some((dl(2), 4)));
    }
}
