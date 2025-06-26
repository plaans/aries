use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::value_order::ValueOrder;

#[derive(Clone, Default, Debug)]
pub struct Max;

impl<Lbl: Label> ValueOrder<Lbl> for Max {
    fn select(&mut self, var: VarRef, model: &Model<Lbl>) -> Lit {
        let ub = model.state.ub(var);
        var.geq(ub)
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
        assert_eq!(Max.select(x, &model), x.geq(1));
        assert_eq!(Max.select(y, &model), y.geq(5));
    }
}
