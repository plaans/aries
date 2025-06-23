use aries::core::IntCst;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::linear::NFLinearLeq;
use aries::model::lang::linear::NFLinearSumItem;
use aries::reif::ReifExpr;

use crate::aries::Post;

/// Linear less or equal constraint.
///
/// `sum(v[i] * c[i]) <= b`
/// where `v[i]` are variables, `b` and `c[i]` constants.
#[derive(Debug)]
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
    use crate::aries::constraint::test::basic_lin_model;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, sum, x, y, c_x, c_y, b) = basic_lin_model();

        let lin_le = LinLe::new(sum, b);
        lin_le.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x * c_x + y * c_y <= b;

        verify_all([x, y], model, verify);
    }
}
