use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::value_order::ValueOrder;

#[derive(Clone, Default, Debug)]
pub struct Min;

impl<Lbl: Label> ValueOrder<Lbl> for Min {
    fn select(&mut self, var: VarRef, model: &Model<Lbl>) -> Lit {
        let lb = model.state.lb(var);
        var.leq(lb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select() {
        let mut model = Model::<String>::new();
        let x = model.new_ivar(0, 1, "x").into();
        let y = model.new_ivar(3, 5, "y").into();
        assert_eq!(Min.select(x, &model), x.leq(0));
        assert_eq!(Min.select(y, &model), y.leq(3));
    }
}
