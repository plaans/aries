use aries::core::IntCst;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::linear::NFLinearSumItem;

use crate::aries::Post;
use crate::aries::constraint::LinGe;
use crate::aries::constraint::LinLe;

/// Linear equality constraint.
///
/// `sum(v[i] * c[i]) = b`
/// where `v[i]` are variables, `b` and `c[i]` constants.
#[derive(Debug)]
pub struct LinEq {
    sum: Vec<NFLinearSumItem>,
    b: IntCst,
}

impl LinEq {
    pub fn new(sum: Vec<NFLinearSumItem>, b: IntCst) -> Self {
        Self { sum, b }
    }

    pub fn sum(&self) -> &Vec<NFLinearSumItem> {
        &self.sum
    }

    pub fn b(&self) -> &IntCst {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for LinEq {
    fn post(&self, model: &mut Model<Lbl>) {
        let lin_le = LinLe::new(self.sum.clone(), self.b);
        let lin_ge = LinGe::new(self.sum.clone(), self.b);
        lin_le.post(model);
        lin_ge.post(model);
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

        let lin_eq = LinEq::new(sum, b);
        lin_eq.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x * c_x + y * c_y == b;

        verify_all([x, y], model, verify);
    }
}
