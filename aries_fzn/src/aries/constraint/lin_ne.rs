use aries::core::IntCst;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::IVar;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::linear::LinearTerm;
use aries::model::lang::linear::NFLinearSumItem;

use crate::aries::Post;

/// Linear not equal constraint.
///
/// `sum(v[i] * c[i]) != b`
/// where `v[i]` are variables, `b` and `c[i]` constants.
#[derive(Debug)]
pub struct LinNe {
    sum: Vec<NFLinearSumItem>,
    b: IntCst,
}

impl LinNe {
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

impl<Lbl: Label> Post<Lbl> for LinNe {
    fn post(&self, model: &mut Model<Lbl>) {
        let above = model.state.new_var(0, 1).geq(1);
        let sum = self.sum.iter().fold(LinearSum::zero(), |accu, elem| {
            accu + LinearTerm::int(elem.factor, IVar::new(elem.var))
        });
        model.enforce_if(above, sum.clone().geq(self.b + 1));
        model.enforce_if(!above, sum.leq(self.b - 1));
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

        let lin_ne = LinNe::new(sum, b);
        lin_ne.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x * c_x + y * c_y != b;

        verify_all([x, y], model, verify);
    }
}
