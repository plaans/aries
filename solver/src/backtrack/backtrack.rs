use crate::backtrack::DecLvl;

pub trait Backtrack {
    fn save_state(&mut self) -> DecLvl;
    fn num_saved(&self) -> u32;
    fn current_decision_level(&self) -> DecLvl {
        DecLvl::from(self.num_saved())
    }
    fn restore_last(&mut self);
    fn restore(&mut self, saved_id: DecLvl) {
        while self.current_decision_level() > saved_id {
            self.restore_last();
        }
    }

    fn reset(&mut self) {
        if self.current_decision_level() > DecLvl::ROOT {
            self.restore(DecLvl::ROOT);
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
