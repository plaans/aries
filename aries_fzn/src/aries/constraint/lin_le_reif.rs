use aries::core::IntCst;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::linear::NFLinearSumItem;

use crate::aries::Post;
use crate::aries::constraint::LinGeHalf;
use crate::aries::constraint::LinLeHalf;
use crate::aries::constraint::Ne;

/// Reified linear less or equal constraint.
///
/// `r <-> sum(v[i] * c[i]) <= ub` where
/// `r` is a boolean variable,
/// `v[i]` are integer variables,
/// `ub` and `c[i]` constants.
#[derive(Debug)]
pub struct LinLeReif {
    sum: Vec<NFLinearSumItem>,
    ub: IntCst,
    r: BVar,
}

impl LinLeReif {
    pub fn new(sum: Vec<NFLinearSumItem>, ub: IntCst, r: BVar) -> Self {
        Self { sum, ub, r }
    }

    pub fn sum(&self) -> &Vec<NFLinearSumItem> {
        &self.sum
    }

    pub fn ub(&self) -> &IntCst {
        &self.ub
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for LinLeReif {
    fn post(&self, model: &mut Model<Lbl>) {
        let not_r = BVar::new(model.state.new_var(0, 1));
        let ne = Ne::new(self.r.int_view(), not_r.int_view());
        let lin_le_half = LinLeHalf::new(self.sum.clone(), self.ub, self.r);
        let lin_ge_half = LinGeHalf::new(self.sum.clone(), self.ub + 1, not_r);
        ne.post(model);
        lin_le_half.post(model);
        lin_ge_half.post(model);
    }
}

#[cfg(test)]
mod tests {
    use crate::aries::constraint::test::basic_lin_model;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, sum, x, y, c_x, c_y, ub) = basic_lin_model();

        let r = model.new_bvar("r".to_string());

        let lin_le_reif = LinLeReif::new(sum, ub, r);
        lin_le_reif.post(&mut model);

        let verify =
            |[x, y, r]: [IntCst; 3]| (r == 1) == (x * c_x + y * c_y <= ub);

        verify_all([x, y, r.into()], model, verify);
    }
}
