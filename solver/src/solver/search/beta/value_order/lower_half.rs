use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::value_order::ValueOrder;

#[derive(Clone, Debug)]
pub struct LowerHalf;

impl<Lbl: Label> ValueOrder<Lbl> for LowerHalf {
    fn select(&mut self, var: VarRef, model: &Model<Lbl>) -> Lit {
        let (lb, ub) = model.state.bounds(var);
        debug_assert!(lb < ub);
        let mid = (lb + ub) / 2;
        var.leq(mid)
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
        let z = model.new_ivar(-5, -3, "z").into();
        assert_eq!(LowerHalf.select(x, &model), x.leq(0));
        assert_eq!(LowerHalf.select(y, &model), y.leq(4));
        assert_eq!(LowerHalf.select(z, &model), z.leq(-4));
    }
}
