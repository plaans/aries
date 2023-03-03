use aries::backtrack::{Backtrack, DecLvl};
use aries::model::extensions::AssignmentExt;
use aries::model::Model;
use aries::solver::solver::search::{Decision, SearchControl};
use aries::solver::solver::stats::Stats;

/// Assigns all value in lexical order to their minimal value.
/// Essentially intended to finish the search once all high-priority variables have been set.
#[derive(Copy, Clone)]
pub struct LexicalMinValue {
    lvl: DecLvl,
}

impl LexicalMinValue {
    pub fn new() -> Self {
        LexicalMinValue { lvl: DecLvl::ROOT }
    }
}

impl Backtrack for LexicalMinValue {
    fn save_state(&mut self) -> DecLvl {
        self.lvl += 1;
        self.lvl
    }

    fn num_saved(&self) -> u32 {
        self.lvl.to_int()
    }

    fn restore_last(&mut self) {
        self.lvl -= 1;
    }
}

impl<L> SearchControl<L> for LexicalMinValue {
    fn next_decision(&mut self, _stats: &Stats, model: &Model<L>) -> Option<Decision> {
        // set the first domain value of the first unset variable
        model
            .state
            .variables()
            .filter_map(|v| {
                let dom = model.var_domain(v);
                if dom.is_bound() {
                    None
                } else {
                    Some(Decision::SetLiteral(v.leq(dom.lb)))
                }
            })
            .next()
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<L> + Send> {
        Box::new(*self)
    }
}
