use crate::backtrack::{Backtrack, DecLvl, DecisionLevelTracker};
use crate::core::Lit;
use crate::model::extensions::AssignmentExt;
use crate::model::Model;
use crate::solver::search::{Decision, SearchControl};
use crate::solver::stats::Stats;
use itertools::Itertools;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

/// A search strategy that selects variable in a random order.
/// Primarily intended for testing purposes.
#[derive(Clone)]
pub struct RandomChoice {
    rng: SmallRng,
    lvl: DecisionLevelTracker,
}

impl RandomChoice {
    pub fn new(seed: u64) -> Self {
        RandomChoice {
            rng: SmallRng::seed_from_u64(seed),
            lvl: Default::default(),
        }
    }
}

impl Backtrack for RandomChoice {
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

impl<L> SearchControl<L> for RandomChoice {
    fn next_decision(&mut self, _stats: &Stats, model: &Model<L>) -> Option<Decision> {
        // set the first domain value of the first unset variable
        let variables = model
            .state
            .variables()
            .filter(|v| {
                if model.state.present(*v) == Some(true) {
                    let dom = model.var_domain(*v);
                    !dom.is_bound()
                } else {
                    false
                }
            })
            .collect_vec();
        if variables.is_empty() {
            return None;
        }
        let var_id = self.rng.gen_range(0..variables.len());
        let var = variables[var_id];
        let (lb, ub) = model.state.bounds(var);
        let upper: bool = self.rng.gen();
        if upper {
            let val = self.rng.gen_range(lb..ub);
            Some(Decision::SetLiteral(Lit::leq(var, val)))
        } else {
            let val = self.rng.gen_range((lb + 1)..=ub);
            Some(Decision::SetLiteral(Lit::geq(var, val)))
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<L> + Send> {
        Box::new(self.clone())
    }
}
