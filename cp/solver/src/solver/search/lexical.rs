use crate::solver::search::{Decision, SearchControl};
use crate::solver::stats::Stats;
use aries_backtrack::{Backtrack, DecLvl, DecisionLevelTracker};
use aries_model::extensions::AssignmentExt;
use aries_model::Model;

/// Assigns all values in lexical order to their minimal value.
/// Essentially intended to finish the search once all high-priority variables have been set.
#[derive(Copy, Clone, Default)]
pub struct LexicalMinValue {
    lvl: DecisionLevelTracker,
}

impl LexicalMinValue {
    pub fn new() -> Self {
        Default::default()
    }
}

impl Backtrack for LexicalMinValue {
    fn save_state(&mut self) -> DecLvl {
        self.lvl.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.lvl.num_saved()
    }

    fn restore_last(&mut self) {
        self.lvl.restore_last()
    }
}

impl<L> SearchControl<L> for LexicalMinValue {
    fn next_decision(&mut self, _stats: &Stats, model: &Model<L>) -> Option<Decision> {
        // set the first domain value of the first unset variable
        model
            .state
            .variables()
            .filter_map(|v| {
                if model.state.present(v) == Some(true) {
                    let dom = model.var_domain(v);
                    if dom.is_bound() {
                        None
                    } else {
                        Some(Decision::SetLiteral(v.leq(dom.lb)))
                    }
                } else {
                    None
                }
            })
            .next()
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<L> + Send> {
        Box::new(*self)
    }
}
