use num_integer::Integer;

use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::value_order::ValueOrder;

#[derive(Clone, Debug)]
pub struct UpperHalf;

impl<Lbl: Label> ValueOrder<Lbl> for UpperHalf {
    fn select(&mut self, var: VarRef, model: &Model<Lbl>) -> Lit {
        let (lb, ub) = model.state.bounds(var);
        let (mid, rem) = (lb + ub).div_mod_floor(&2);
        var.geq(mid + rem)
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
        assert_eq!(UpperHalf.select(x, &model), x.geq(1));
        assert_eq!(UpperHalf.select(y, &model), y.geq(4));
    }
}
