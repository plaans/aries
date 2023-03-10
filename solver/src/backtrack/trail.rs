use crate::backtrack::DecLvl;

/// A trail consists of a sequence of events typically representing the changes
/// to a data structure.
/// The purpose of this structure is to allow undoing the changes in order to restore a
/// previous state.
///
/// It supports save points, on which one may to backtrack.
#[derive(Clone)]
pub struct Trail<Event> {
    pub trail: Vec<Event>,
    pub saved_states: Vec<usize>,
}

impl<Event> Trail<Event> {
    pub fn new() -> Self {
        Trail {
            trail: vec![],
            saved_states: vec![],
        }
    }

    pub fn push(&mut self, e: Event) {
        self.trail.push(e);
    }

    /// Removes and returns the last event within the last saved state.
    ///
    /// # Panic
    ///
    /// Panic if there is no event to remove within the current decision level
    pub fn pop_within_level(&mut self) -> Option<Event> {
        // check that we can undo an event without changing the backtrack level
        assert!(self.trail.len() > self.saved_states.last().copied().unwrap_or(0));
        self.trail.pop()
    }

    pub fn save_state(&mut self) -> DecLvl {
        self.saved_states.push(self.trail.len());
        DecLvl::from(self.saved_states.len())
    }

    pub fn num_saved(&self) -> u32 {
        self.saved_states.len() as u32
    }

    pub fn current_decision_level(&self) -> DecLvl {
        DecLvl::from(self.num_saved())
    }

    fn undo_last_with(&mut self, mut f: impl FnMut(Event)) {
        let last = self.trail.pop().expect("No event left");
        f(last)
    }

    pub fn restore_last_with(&mut self, mut f: impl FnMut(Event)) {
        let last_index = self.saved_states.pop().expect("No saved state");
        while self.trail.len() > last_index {
            self.undo_last_with(&mut f)
        }
    }

    pub fn restore(&mut self, saved_state: u32, mut f: impl FnMut(Event)) {
        while self.num_saved() > saved_state {
            self.restore_last_with(&mut f)
        }
    }
}

impl<Event> Default for Trail<Event> {
    fn default() -> Self {
        Self::new()
    }
}
