use aries::core::IntCst;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::lang::BVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::LinGeHalf;
use crate::aries::constraint::LinLeHalf;
use crate::aries::Post;

/// Half reified linear equality constraint.
///
/// `r -> sum(v[i] * c[i]) = b` where
/// `r` is a boolean variable,
/// `v[i]` are integer variables,
/// `b` and `c[i]` constants.
#[derive(Debug)]
pub struct LinEqHalf {
    sum: Vec<NFLinearSumItem>,
    b: IntCst,
    r: BVar,
}

impl LinEqHalf {
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

impl<Lbl: Label> Post<Lbl> for LinEqHalf {
    fn post(&self, model: &mut Model<Lbl>) {
        let lin_le_half = LinLeHalf::new(self.sum.clone(), self.b, self.r);
        let lin_ge_half = LinGeHalf::new(self.sum.clone(), self.b, self.r);
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
        let (mut model, sum, x, y, c_x, c_y, b) = basic_lin_model();

        let r = model.new_bvar("r".to_string());

        let lin_eq_half = LinEqHalf::new(sum, b, r);
        lin_eq_half.post(&mut model);

        let verify = |[x, y, r]: [IntCst; 3]| r == 0 || x * c_x + y * c_y == b;

        verify_all([x, y, r.into()], model, verify);
    }
}
