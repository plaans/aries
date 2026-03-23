use aries::core::IntCst;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::linear::NFLinearSumItem;

use crate::aries::Post;
use crate::aries::constraint::AndReif;
use crate::aries::constraint::LinGeReif;
use crate::aries::constraint::LinLeReif;

/// Reified linear equality constraint.
///
/// `r <-> sum(v[i] * c[i]) = b` where
/// `r` is a boolean variable,
/// `v[i]` are integer variables,
/// `b` and `c[i]` constants.
#[derive(Debug)]
pub struct LinEqReif {
    sum: Vec<NFLinearSumItem>,
    b: IntCst,
    r: BVar,
}

impl LinEqReif {
    pub fn new(sum: Vec<NFLinearSumItem>, b: IntCst, r: BVar) -> Self {
        Self { sum, b, r }
    }

    pub fn sum(&self) -> &Vec<NFLinearSumItem> {
        &self.sum
    }

    pub fn b(&self) -> &IntCst {
        &self.b
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for LinEqReif {
    fn post(&self, model: &mut Model<Lbl>) {
        let r1 = BVar::new(model.state.new_var(0, 1));
        let r2 = BVar::new(model.state.new_var(0, 1));
        let lin_le_reif = LinLeReif::new(self.sum.clone(), self.b, r1);
        let lin_ge_reif = LinGeReif::new(self.sum.clone(), self.b, r2);
        let and_reif = AndReif::new(vec![r1, r2], self.r);
        lin_le_reif.post(model);
        lin_ge_reif.post(model);
        and_reif.post(model);
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

        let r = model.new_bvar("r".to_string());

        let lin_eq_half = LinEqReif::new(sum, b, r);
        lin_eq_half.post(&mut model);

        let verify =
            |[x, y, r]: [IntCst; 3]| (r == 1) == (x * c_x + y * c_y == b);

        verify_all([x, y, r.into()], model, verify);
    }
}
