use crate::backtrack::{Backtrack, DecLvl};
use crate::core::IntCst;
use crate::core::state::{Conflict, Explainer};
use crate::model::Model;
use crate::model::extensions::SavedAssignment;
use crate::solver::search::{Brancher, Decision, SearchControl};
use crate::solver::stats::Stats;
use itertools::Itertools;
use std::sync::Arc;

/// A trait that provides extension methods for branchers
pub trait CombinatorExt<L> {
    /// Creates a brancher that will systematically ask the `self` brancher for a decision.
    /// If the `fallback` brancher has no decisions left, it will provide the result  
    fn and_then(self, fallback: Brancher<L>) -> Brancher<L>;

    /// Creates a brancher that extends `self` to have geometric restarts.
    fn with_restarts(self, allowed_conflicts: u64, increase_ratio: f32) -> Brancher<L>;
}

impl<L: 'static> CombinatorExt<L> for Brancher<L> {
    fn and_then(self, second: Brancher<L>) -> Brancher<L> {
        Box::new(AndThen::new(self, second))
    }

    fn with_restarts(self, allowed_conflicts: u64, increase_ratio: f32) -> Brancher<L> {
        Box::new(WithGeomRestart::new(allowed_conflicts, increase_ratio, self))
    }
}

/// A brancher that will systematically ask the `first` brancher for a decision.
/// If the `first` brancher has no decisions left, it will provide the result  
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

    fn conflict(
        &mut self,
        clause: &Conflict,
        model: &Model<L>,
        explainer: &mut dyn Explainer,
        backtrack_level: DecLvl,
    ) {
        self.first.conflict(clause, model, explainer, backtrack_level);
        self.second.conflict(clause, model, explainer, backtrack_level);
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

/// A brancher that stop providing decisions once a conflict has been found.
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

    fn conflict(
        &mut self,
        _clause: &Conflict,
        _model: &Model<L>,
        _explainer: &mut dyn Explainer,
        _backtrack_level: DecLvl,
    ) {
        self.active = false;
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

/// A brancher that extends a `brancher` with geometric restarts.
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

    fn conflict(
        &mut self,
        clause: &Conflict,
        model: &Model<L>,
        explainer: &mut dyn Explainer,
        backtrack_level: DecLvl,
    ) {
        self.brancher.conflict(clause, model, explainer, backtrack_level)
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

/// A solver that alternates between the given strategies in a round-robin fashion.
pub struct RoundRobin<L> {
    /// Number of conflicts before switching to the next
    num_conflicts_per_period: u64,
    /// Factor by witch to multiply the number of conflicts/period  after a switch
    increase_factor: f64,
    num_conflicts_since_switch: u64,
    /// Index of the current brancher.
    current_idx: usize,
    branchers: Vec<Brancher<L>>,
}

impl<L> RoundRobin<L> {
    pub fn new(num_conflicts_per_period: u64, increase_factor: f64, branchers: Vec<Brancher<L>>) -> Self {
        RoundRobin {
            num_conflicts_per_period,
            increase_factor,
            num_conflicts_since_switch: 0,
            current_idx: 0,
            branchers,
        }
    }
    fn current(&self) -> &Brancher<L> {
        &self.branchers[self.current_idx]
    }
    fn current_mut(&mut self) -> &mut Brancher<L> {
        &mut self.branchers[self.current_idx]
    }
}

impl<L> Backtrack for RoundRobin<L> {
    fn save_state(&mut self) -> DecLvl {
        self.current_mut().save_state()
    }

    fn num_saved(&self) -> u32 {
        self.current().num_saved()
    }

    fn restore_last(&mut self) {
        self.current_mut().restore_last();

        // we are at the ROOT, check if we should switch to the next brancher
        if self.num_saved() == 0 && self.num_conflicts_since_switch >= self.num_conflicts_per_period {
            self.current_idx = (self.current_idx + 1) % self.branchers.len();
            self.num_conflicts_since_switch = 0;
            self.num_conflicts_per_period = (self.num_conflicts_per_period as f64 * self.increase_factor) as u64;
        }
    }
}

impl<L: 'static> SearchControl<L> for RoundRobin<L> {
    fn next_decision(&mut self, stats: &Stats, model: &Model<L>) -> Option<Decision> {
        self.current_mut().next_decision(stats, model)
    }

    fn import_vars(&mut self, model: &Model<L>) {
        self.current_mut().import_vars(model)
    }

    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: Arc<SavedAssignment>) {
        self.current_mut().new_assignment_found(objective_value, assignment)
    }

    fn pre_save_state(&mut self, _model: &Model<L>) {
        self.current_mut().pre_save_state(_model);
    }

    fn pre_conflict_analysis(&mut self, _model: &Model<L>) {
        self.current_mut().pre_conflict_analysis(_model);
    }

    fn conflict(
        &mut self,
        clause: &Conflict,
        model: &Model<L>,
        explainer: &mut dyn Explainer,
        backtrack_level: DecLvl,
    ) {
        self.num_conflicts_since_switch += 1;
        self.current_mut().conflict(clause, model, explainer, backtrack_level)
    }

    fn clone_to_box(&self) -> Brancher<L> {
        Box::new(Self {
            num_conflicts_per_period: self.num_conflicts_per_period,
            increase_factor: self.increase_factor,
            num_conflicts_since_switch: self.num_conflicts_since_switch,
            current_idx: self.current_idx,
            branchers: self.branchers.iter().map(|b| b.clone_to_box()).collect_vec(),
        })
    }
}
