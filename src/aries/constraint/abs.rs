use aries::model::lang::linear::NFLinearSumItem;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::LinEq;
use crate::aries::constraint::Max;
use crate::aries::Post;

/// Represent the constraint:
/// `b = abs(a)`
#[derive(Debug)]
pub struct Abs {
    a: IVar,
    b: IVar,
}

impl Abs {
    pub fn new(a: IVar, b: IVar) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &IVar {
        &self.a
    }

    pub fn b(&self) -> &IVar {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for Abs {
    fn post(&self, model: &mut Model<Lbl>) {
        let (lb, ub) = model.state.bounds(self.a.into());

        let minus_a = model.state.new_var(-ub, -lb);
        let minus_a = IVar::new(minus_a);

        let plus_a = self.a;

        let sum = vec![
            NFLinearSumItem {
                var: plus_a.into(),
                factor: 1,
            },
            NFLinearSumItem {
                var: minus_a.into(),
                factor: 1,
            },
        ];

        let lin_eq = LinEq::new(sum, 0);
        lin_eq.post(model);

        // minus_a is now equal to -plus_a

        let max = Max::new(vec![minus_a, plus_a], self.b);
        max.post(model);
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_2;
    use crate::aries::constraint::test::verify_all_2;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y) = basic_int_model_2();

        let abs = Abs::new(x, y);
        abs.post(&mut model);

        let verify = |x: IntCst, y| y == x.abs();

        verify_all_2(x, y, model, verify);
    }
}
