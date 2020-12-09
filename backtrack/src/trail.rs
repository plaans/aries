
/// A trail consists of a sequence of events typically representing the changes
/// to a data structure.
/// The purpose of this structure is to allow undoing the changes in order to restore a
/// previous state.
///
/// It supports save points, on which one may to backtrack.
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
        if !self.saved_states.is_empty() {
            // only save things if we have an initial saved.
            // Otherwise, there is no point in maintaining it as it cannot be undone
            self.trail.push(e);
        }
    }

    pub fn save_state(&mut self) -> u32 {
        self.saved_states.push(self.trail.len());
        self.saved_states.len() as u32 - 1
    }

    pub fn num_saved(&self) -> u32 {
        self.saved_states.len() as u32
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
