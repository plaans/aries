pub trait Backtrack {
    fn save_state(&mut self) -> u32;
    fn num_saved(&self) -> u32;
    fn restore_last(&mut self);
    fn restore(&mut self, saved_id: u32) {
        while self.num_saved() > saved_id {
            self.restore_last();
        }
    }

    fn reset(&mut self) {
        if self.num_saved() > 0 {
            self.restore(0);
        }
    }
}

pub trait BacktrackWith: Backtrack {
    type Event;

    fn restore_last_with<F: FnMut(&Self::Event)>(&mut self, callback: F);

    fn restore_with<F: FnMut(&Self::Event)>(&mut self, saved_id: u32, mut callback: F) {
        while self.num_saved() > saved_id {
            self.restore_last_with(&mut callback);
        }
    }
}
