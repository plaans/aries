use aries_solver::core::IntCst;
use aries_solver::lang::linear::ScaledVar;
use aries_solver::model::Label;
use aries_solver::model::Model;
use aries_solver::prelude::LinSum;

use crate::aries::Post;

/// Linear less or equal constraint.
///
/// `sum(v[i] * c[i]) <= b`
/// where `v[i]` are variables, `b` and `c[i]` constants.
#[derive(Debug)]
pub struct LinLe {
    items: Vec<ScaledVar>,
    ub: IntCst,
}

impl LinLe {
    pub fn new(items: Vec<ScaledVar>, ub: IntCst) -> Self {
        Self { items, ub }
    }

    pub fn items(&self) -> &Vec<ScaledVar> {
        &self.items
    }

    pub fn ub(&self) -> &IntCst {
        &self.ub
    }
}

impl<Lbl: Label> Post<Lbl> for LinLe {
    fn post(&self, model: &mut Model<Lbl>) {
        let sum = LinSum::new(0, self.items.iter().copied());
        model.enforce(sum.leq(self.ub), []);
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
