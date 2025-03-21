use aries::core::IntCst;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::LinGe;
use crate::aries::constraint::LinLe;
use crate::aries::Post;

/// Represent the constraint:
/// `sum(v_i * c_i) = b`
///
/// where `v_i` are variables, b and `c_i` constants
pub struct LinEq {
    items: Vec<NFLinearSumItem>,
    b: IntCst,
}

impl LinEq {
    pub fn new(items: Vec<NFLinearSumItem>, b: IntCst) -> Self {
        Self { items, b }
    }

    pub fn items(&self) -> &Vec<NFLinearSumItem> {
        &self.items
    }

    pub fn b(&self) -> &IntCst {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for LinEq {
    fn post(&self, model: &mut Model<Lbl>) {
        let lin_le = LinLe::new(self.items.clone(), self.b);
        let lin_ge = LinGe::new(self.items.clone(), self.b);
        lin_le.post(model);
        lin_ge.post(model);
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

        let lin_eq = LinEq::new(sum, b);
        lin_eq.post(&mut model);

        let check = |x, y| x*c_x + y*c_y == b;

        verify_all_2(x, y, model, check);
    }
}
