use crate::backtrack::Backtrack;
use crate::backtrack::DecLvl;
use crate::backtrack::DecisionLevelTracker;
use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::SearchControl;
use crate::solver::stats::Stats;

use super::Brancher;
use super::Decision;

#[derive(Clone, Copy, Default, Debug)]
pub enum VarOrder {
    #[default]
    Lexical,
}

impl VarOrder {
    /// Return the variable to branch on or None if no one is available.
    pub fn select<Lbl: Label>(&self, model: &Model<Lbl>) -> Option<VarRef> {
        match self {
            VarOrder::Lexical => model.state.variables().filter(|v| !model.state.is_bound(*v)).next(),
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub enum ValueOrder {
    #[default]
    Min,
    Max,
}

impl ValueOrder {
    /// Return the literal to decide for the given variable.
    pub fn select<Lbl: Label>(&self, var: VarRef, model: &Model<Lbl>) -> Lit {
        let (lb, ub) = model.state.bounds(var);
        debug_assert!(lb < ub);
        match self {
            ValueOrder::Min => var.leq(lb),
            ValueOrder::Max => var.geq(ub),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BetaBrancher {
    lvl: DecisionLevelTracker,
    var_order: VarOrder,
    value_order: ValueOrder,
}

impl BetaBrancher {
    pub fn new(var_order: VarOrder, value_order: ValueOrder) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Prepare a basic model for the tests.
    /// Use it as follows.
    /// ```
    /// let (model, a, b, c) = basic_model();
    /// ```
    ///
    /// It has three variables:
    ///  - a in \[2,2\]
    ///  - b in \[3,9\]
    ///  - c in \[0,1\]
    fn basic_model() -> (Model<String>, VarRef, VarRef, VarRef) {
        let mut model = Model::new();
        let a = model.new_ivar(2, 2, "a").into();
        let b = model.new_ivar(3, 9, "b").into();
        let c = model.new_bvar("c").into();
        (model, a, b, c)
    }

    #[test]
    fn min() {
        let (model, _a, b, c) = basic_model();
        let min = ValueOrder::Min;
        // No assert on var a since it is bound
        assert_eq!(min.select(b, &model), b.leq(3));
        assert_eq!(min.select(c, &model), c.leq(0));
    }

    #[test]
    fn max() {
        let (model, _a, b, c) = basic_model();
        let max = ValueOrder::Max;
        // No assert on var a since it is bound
        assert_eq!(max.select(b, &model), b.geq(9));
        assert_eq!(max.select(c, &model), c.geq(1));
    }

    #[test]
    fn lexical() {
        let (model, _a, b, _c) = basic_model();
        let lexical = VarOrder::Lexical;
        assert_eq!(lexical.select(&model), Some(b));
    }
}
