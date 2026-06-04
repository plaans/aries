use aries::lang::Var;
use aries::lang::linear::ScaledVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;
use crate::aries::constraint::LinEq;
use crate::aries::constraint::Max;

/// Absolute value constraint.
///
/// `b = abs(a)`
#[derive(Debug)]
pub struct Abs {
    a: Var,
    b: Var,
}

impl Abs {
    pub fn new(a: Var, b: Var) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Var {
        &self.a
    }

    pub fn b(&self) -> &Var {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for Abs {
    fn post(&self, model: &mut Model<Lbl>) {
        let (lb, ub) = model.state.bounds(self.a);

        let minus_a = model.state.new_var(-ub, -lb);

        let plus_a = self.a;

        let sum = vec![
            ScaledVar {
                var: plus_a,
                factor: 1,
            },
            ScaledVar {
                var: minus_a,
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
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y) = basic_int_model_2();

        let abs = Abs::new(x, y);
        abs.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| y == x.abs();

        verify_all([x, y], model, verify);
    }
}
