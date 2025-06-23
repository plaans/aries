use aries::core::IntCst;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::IVar;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::linear::LinearTerm;
use aries::model::lang::linear::NFLinearSumItem;

use crate::aries::Post;

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
        // r => sum(v[i] * c[i]) <= ub
        let sum = self.sum.iter().fold(LinearSum::zero(), |accu, elem| {
            accu + LinearTerm::int(elem.factor, IVar::new(elem.var))
        });
        let constraint = sum.leq(self.ub);
        model.enforce_if(self.r.true_lit(), constraint);
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
