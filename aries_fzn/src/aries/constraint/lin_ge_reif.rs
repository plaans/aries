use aries::core::IntCst;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::lang::BVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::LinLeReif;
use crate::aries::Post;

/// Reified linear greater or equal constraint.
///
/// `r <-> sum(v[i] * c[i]) >= lb` where
/// `r` is a boolean variable,
/// `v[i]` are integer variables,
/// `lb` and `c[i]` constants.
#[derive(Debug)]
pub struct LinGeReif {
    sum: Vec<NFLinearSumItem>,
    lb: IntCst,
    r: BVar,
}

impl LinGeReif {
    pub fn new(sum: Vec<NFLinearSumItem>, lb: IntCst, r: BVar) -> Self {
        Self { sum, lb, r }
    }

    pub fn sum(&self) -> &Vec<NFLinearSumItem> {
        &self.sum
    }

    pub fn lb(&self) -> &IntCst {
        &self.lb
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for LinGeReif {
    fn post(&self, model: &mut Model<Lbl>) {
        // sum(v[i] * c[i]) >= lb iff sum(v[i] * -c[i]) <= -lb
        let sum = self.sum.iter().cloned().map(|item| -item).collect();
        let lin_le_reif = LinLeReif::new(sum, -self.lb, self.r);
        lin_le_reif.post(model);
    }
}

#[cfg(test)]
mod tests {
    use crate::aries::constraint::test::basic_lin_model;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, sum, x, y, c_x, c_y, lb) = basic_lin_model();

        let r = model.new_bvar("r".to_string());

        let lin_ge_reif = LinGeReif::new(sum, lb, r);
        lin_ge_reif.post(&mut model);

        let verify =
            |[x, y, r]: [IntCst; 3]| (r == 1) == (x * c_x + y * c_y >= lb);

        verify_all([x, y, r.into()], model, verify);
    }
}
