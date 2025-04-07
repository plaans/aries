use aries::core::IntCst;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::LinLe;
use crate::aries::Post;

/// Linear greater or equal constraint.
///
/// `sum(v[i] * c[i]) >= b`
/// where `v[i]` are variables, `b` and `c[i]` constants.
#[derive(Debug)]
pub struct LinGe {
    items: Vec<NFLinearSumItem>,
    lb: IntCst,
}

impl LinGe {
    pub fn new(items: Vec<NFLinearSumItem>, lb: IntCst) -> Self {
        Self { items, lb }
    }

    pub fn items(&self) -> &Vec<NFLinearSumItem> {
        &self.items
    }

    pub fn lb(&self) -> &IntCst {
        &self.lb
    }
}

impl<Lbl: Label> Post<Lbl> for LinGe {
    fn post(&self, model: &mut Model<Lbl>) {
        let minus = |i: &NFLinearSumItem| NFLinearSumItem {
            var: i.var,
            factor: -i.factor,
        };
        let lin_le =
            LinLe::new(self.items.iter().map(minus).collect(), -self.lb);
        lin_le.post(model);
    }
}

#[cfg(test)]
mod tests {
    use crate::aries::constraint::test::basic_lin_model;
    use crate::aries::constraint::test::verify_all_2;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, sum, x, y, c_x, c_y, b) = basic_lin_model();

        let lin_ge = LinGe::new(sum, b);
        lin_ge.post(&mut model);

        let verify = |x, y| x * c_x + y * c_y >= b;

        verify_all_2(x, y, model, verify);
    }
}
