use aries::core::INT_CST_MAX;
use aries::core::IntCst;
use aries::core::state::Term;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::linear::NFLinearSumItem;

use crate::aries::Post;
use crate::aries::constraint::LinLe;

/// Half reified linear less or equal constraint.
///
/// `r -> sum(v[i] * c[i]) <= ub` where
/// `r` is a boolean variable,
/// `v[i]` are integer variables,
/// `ub` and `c[i]` constants.
#[derive(Debug)]
pub struct LinLeHalf {
    sum: Vec<NFLinearSumItem>,
    ub: IntCst,
    r: BVar,
}

impl LinLeHalf {
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

impl<Lbl: Label> Post<Lbl> for LinLeHalf {
    fn post(&self, model: &mut Model<Lbl>) {
        // Big M
        let m = INT_CST_MAX;

        // sum(v[i] * c[i]) + r * M <= ub + M
        let mut sum = self.sum.clone();
        sum.push(NFLinearSumItem {
            var: self.r.variable(),
            factor: m,
        });

        let lin_le = LinLe::new(sum, self.ub + m);
        lin_le.post(model);
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

        let lin_le_half = LinLeHalf::new(sum, ub, r);
        lin_le_half.post(&mut model);

        let verify = |[x, y, r]: [IntCst; 3]| r == 0 || x * c_x + y * c_y <= ub;

        verify_all([x, y, r.into()], model, verify);
    }
}
