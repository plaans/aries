use aries::core::IntCst;
use aries::model::lang::linear::NFLinearLeq;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::Label;
use aries::model::Model;
use aries::reif::ReifExpr;

use crate::aries::Post;

/// Represent the constraint:
/// `sum(v_i * c_i) <= ub`
///
/// where `v_i` are variables, ub and `c_i` constants
pub struct LinLe {
    items: Vec<NFLinearSumItem>,
    ub: IntCst,
}

impl LinLe {
    pub fn new(items: Vec<NFLinearSumItem>, ub: IntCst) -> Self {
        Self { items, ub }
    }

    pub fn items(&self) -> &Vec<NFLinearSumItem> {
        &self.items
    }

    pub fn ub(&self) -> &IntCst {
        &self.ub
    }
}

impl<Lbl: Label> Post<Lbl> for LinLe {
    fn post(&self, model: &mut Model<Lbl>) {
        let leq = NFLinearLeq {
            sum: self.items.clone(),
            upper_bound: self.ub,
        };
        let reif_expr = ReifExpr::Linear(leq);
        model.enforce(reif_expr, []);
    }
}

#[cfg(test)]
mod tests {
    use crate::aries::constraint::test::verify_all_2;
    use crate::aries::constraint::test::basic_lin_model;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, sum, x, y, c_x, c_y, b) = basic_lin_model();

        let lin_le = LinLe::new(sum, b);
        lin_le.post(&mut model);

        let check = |x, y| x*c_x + y*c_y <= b;

        verify_all_2(x, y, model, check);
    }
}
