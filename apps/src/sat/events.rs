use std::mem;

use super::all::*;
use crate::collection::index_map::ToIndex;

pub enum Event {
    EmptyDom(BVar),
    Assigned(Lit),
}

trait IndexableUnion {
    fn case(self) -> u8;
    fn payload(self) -> usize;
}

impl IndexableUnion for Event {
    fn case(self) -> u8 {
        match self {
            Event::EmptyDom(_) => 0,
            Event::Assigned(_) => 1,
        }
    }

    fn payload(self) -> usize {
        match self {
            Event::EmptyDom(v) => v.to_index(),
            Event::Assigned(lit) => lit.to_index(),
        }
    }
}

trait Priority {
    fn priority(e: Self) -> u8;
}

impl Priority for Event {
    fn priority(e: Self) -> u8 {
        match e {
            Event::EmptyDom(_) => 1,
            Event::Assigned(_) => 2,
        }
    }
}

trait EventQueue<E> {
    fn enqueue(&mut self, e: E);
    fn dequeue(&mut self) -> Option<E>;
}

trait Triggers<E, F> {
    fn record_trigger(&mut self, e: &E, trigger: F);
    fn get_and_clear_triggers(&mut self, e: &E) -> Vec<F>;
}

type TriggerMap<F> = Vec<Vec<F>>;

struct EventHandler<E, F> {
    queue: Vec<E>,
    triggers: [TriggerMap<F>; 8],
}

impl<E: Priority, F> EventQueue<E> for EventHandler<E, F> {
    fn enqueue(&mut self, e: E) {
        self.queue.push(e)
    }

    fn dequeue(&mut self) -> Option<E> {
        self.queue.pop()
    }
}

impl<E: IndexableUnion + Copy, F> Triggers<E, F> for EventHandler<E, F> {
    fn record_trigger(&mut self, e: &E, trigger: F) {
        let case = e.case();
        let payload = e.payload();
        debug_assert!(case < 8);
        let map = &mut self.triggers[case as usize];
        let events = &mut map[payload];
        events.push(trigger)
    }

    fn get_and_clear_triggers(&mut self, e: &E) -> Vec<F> {
        let case = e.case() as usize;
        debug_assert!(case < 8);

        let map = &mut self.triggers[case as usize];
        mem::replace(&mut map[case], Vec::new())
    }
}
