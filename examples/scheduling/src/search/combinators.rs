use aries::backtrack::{Backtrack, DecLvl};
use aries::core::state::{Conflict, Explainer};
use aries::core::{IntCst, Lit};
use aries::model::extensions::SavedAssignment;
use aries::model::Model;
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
use std::sync::Arc;

pub type Brancher<L> = Box<dyn SearchControl<L> + Send>;

pub trait CombExt<L> {
    fn and_then(self, brancher: Brancher<L>) -> Brancher<L>;
    fn with_restarts(self, allowed_conflicts: u64, increase_ratio: f32) -> Brancher<L>;
}

impl<L: 'static> CombExt<L> for Brancher<L> {
    fn and_then(self, second: Brancher<L>) -> Brancher<L> {
        Box::new(AndThen::new(self, second))
    }

    fn with_restarts(self, allowed_conflicts: u64, increase_ratio: f32) -> Brancher<L> {
        Box::new(WithGeomRestart::new(allowed_conflicts, increase_ratio, self))
    }
}

pub struct AndThen<L> {
    first: Brancher<L>,
    second: Brancher<L>,
}

impl<L> AndThen<L> {
    pub fn new(first: Brancher<L>, second: Brancher<L>) -> Self {
        AndThen { first, second }
    }
}

impl<L> Backtrack for AndThen<L> {
    fn save_state(&mut self) -> DecLvl {
        self.first.save_state();
        self.second.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.second.num_saved()
    }

    fn restore_last(&mut self) {
        self.first.restore_last();
        self.second.restore_last();
    }
}

impl<L: 'static> SearchControl<L> for AndThen<L> {
    fn next_decision(&mut self, stats: &Stats, model: &Model<L>) -> Option<Decision> {
        self.first
            .next_decision(stats, model)
            .or_else(|| self.second.next_decision(stats, model))
    }

    fn import_vars(&mut self, model: &Model<L>) {
        self.first.import_vars(model);
        self.second.import_vars(model);
    }

    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: Arc<SavedAssignment>) {
        self.first.new_assignment_found(objective_value, assignment.clone());
        self.second.new_assignment_found(objective_value, assignment);
    }

    fn conflict(&mut self, clause: &Conflict, model: &Model<L>, explainer: &mut dyn Explainer) {
        self.first.conflict(clause, model, explainer);
        self.second.conflict(clause, model, explainer);
    }

    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model<L>) {
        self.first.asserted_after_conflict(lit, model);
        self.second.asserted_after_conflict(lit, model);
    }

    fn pre_save_state(&mut self, model: &Model<L>) {
        self.first.pre_save_state(model);
        self.second.pre_save_state(model)
    }

    fn pre_conflict_analysis(&mut self, model: &Model<L>) {
        self.first.pre_conflict_analysis(model);
        self.second.pre_conflict_analysis(model)
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<L> + Send> {
        Box::new(AndThen {
            first: self.first.clone_to_box(),
            second: self.second.clone_to_box(),
        })
    }
}

pub struct UntilFirstConflict<L> {
    active: bool,
    brancher: Brancher<L>,
}

impl<L> UntilFirstConflict<L> {
    pub fn new(brancher: Brancher<L>) -> Self {
        UntilFirstConflict { active: true, brancher }
    }
}

impl<L> Backtrack for UntilFirstConflict<L> {
    fn save_state(&mut self) -> DecLvl {
        self.brancher.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.brancher.num_saved()
    }

    fn restore_last(&mut self) {
        self.brancher.restore_last();
    }
}

impl<L: 'static> SearchControl<L> for UntilFirstConflict<L> {
    fn next_decision(&mut self, stats: &Stats, model: &Model<L>) -> Option<Decision> {
        if self.active {
            self.brancher.next_decision(stats, model)
        } else {
            None
        }
    }

    fn import_vars(&mut self, model: &Model<L>) {
        if self.active {
            self.brancher.import_vars(model)
        }
    }

    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: Arc<SavedAssignment>) {
        if self.active {
            self.brancher.new_assignment_found(objective_value, assignment)
        }
    }

    fn conflict(&mut self, _clause: &Conflict, _model: &Model<L>, _explainer: &mut dyn Explainer) {
        self.active = false;
    }

    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model<L>) {
        if self.active {
            self.brancher.asserted_after_conflict(lit, model)
        }
    }

    fn pre_save_state(&mut self, model: &Model<L>) {
        if self.active {
            self.brancher.pre_save_state(model);
        }
    }

    fn pre_conflict_analysis(&mut self, model: &Model<L>) {
        if self.active {
            self.brancher.pre_conflict_analysis(model);
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<L> + Send> {
        Box::new(UntilFirstConflict {
            active: self.active,
            brancher: self.brancher.clone_to_box(),
        })
    }
}

pub struct WithGeomRestart<L> {
    allowed_conflicts: u64,
    increase_ratio_for_allowed_conflict: f32,
    conflicts_at_last_restart: u64,
    brancher: Brancher<L>,
}

impl<L> WithGeomRestart<L> {
    pub fn new(allowed_conflicts: u64, increase_ratio: f32, brancher: Brancher<L>) -> Self {
        WithGeomRestart {
            allowed_conflicts,
            increase_ratio_for_allowed_conflict: increase_ratio,
            conflicts_at_last_restart: 0,
            brancher,
        }
    }
}

impl<L> Backtrack for WithGeomRestart<L> {
    fn save_state(&mut self) -> DecLvl {
        self.brancher.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.brancher.num_saved()
    }

    fn restore_last(&mut self) {
        self.brancher.restore_last()
    }
}

impl<L: 'static> SearchControl<L> for WithGeomRestart<L> {
    fn next_decision(&mut self, stats: &Stats, model: &Model<L>) -> Option<Decision> {
        if stats.num_conflicts() - self.conflicts_at_last_restart >= self.allowed_conflicts {
            // we have exceeded the number of allowed conflict, time for a restart
            self.conflicts_at_last_restart = stats.num_conflicts();
            // increase the number of allowed conflicts
            self.allowed_conflicts = (self.allowed_conflicts as f32 * self.increase_ratio_for_allowed_conflict) as u64;
            Some(Decision::Restart)
        } else {
            self.brancher.next_decision(stats, model)
        }
    }

    fn import_vars(&mut self, model: &Model<L>) {
        self.brancher.import_vars(model)
    }

    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: Arc<SavedAssignment>) {
        self.brancher.new_assignment_found(objective_value, assignment)
    }

    fn conflict(&mut self, clause: &Conflict, model: &Model<L>, explainer: &mut dyn Explainer) {
        self.brancher.conflict(clause, model, explainer)
    }

    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model<L>) {
        self.brancher.asserted_after_conflict(lit, model)
    }

    fn pre_save_state(&mut self, model: &Model<L>) {
        self.brancher.pre_save_state(model);
    }

    fn pre_conflict_analysis(&mut self, model: &Model<L>) {
        self.brancher.pre_conflict_analysis(model);
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<L> + Send> {
        Box::new(WithGeomRestart {
            allowed_conflicts: self.allowed_conflicts,
            increase_ratio_for_allowed_conflict: self.increase_ratio_for_allowed_conflict,
            conflicts_at_last_restart: self.conflicts_at_last_restart,
            brancher: self.brancher.clone_to_box(),
        })
    }
}
