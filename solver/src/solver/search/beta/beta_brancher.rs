use crate::backtrack::Backtrack;
use crate::backtrack::DecLvl;
use crate::backtrack::DecisionLevelTracker;
use crate::core::state::Conflict;
use crate::core::state::Explainer;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::value_order::ValueOrder;
use crate::solver::search::beta::value_order::ValueOrderKind;
use crate::solver::search::beta::var_order::VarOrder;
use crate::solver::search::beta::var_order::VarOrderKind;
use crate::solver::search::Brancher;
use crate::solver::search::Decision;
use crate::solver::search::SearchControl;
use crate::solver::stats::Stats;

/// Brancher for benchamrk, it is designed to be modular.
#[derive(Clone, Debug)]
pub struct BetaBrancher {
    lvl: DecisionLevelTracker,
    var_order: VarOrderKind,
    value_order: ValueOrderKind,
}

impl BetaBrancher {
    pub fn new(var_order: VarOrderKind, value_order: ValueOrderKind) -> Self {
        let lvl = DecisionLevelTracker::new();
        Self {
            var_order,
            value_order,
            lvl,
        }
    }
}

impl<Lbl: Label> SearchControl<Lbl> for BetaBrancher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model<Lbl>) -> Option<Decision> {
        let var = self.var_order.select(model)?;
        let lit = self.value_order.select(var, model);
        Some(Decision::SetLiteral(lit))
    }

    fn clone_to_box(&self) -> Brancher<Lbl> {
        Box::new(self.clone())
    }

    fn conflict(
        &mut self,
        clause: &Conflict,
        model: &Model<Lbl>,
        explainer: &mut dyn Explainer,
        backtrack_level: DecLvl,
    ) {
        match &mut self.var_order {
            VarOrderKind::Activity(activity) => activity.conflict(clause, model, explainer, backtrack_level),
            VarOrderKind::Lexical(lexical) => lexical.conflict(clause, model, explainer, backtrack_level),
            VarOrderKind::FirstFail(first_fail) => first_fail.conflict(clause, model, explainer, backtrack_level),
        }
    }
}

impl Backtrack for BetaBrancher {
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
