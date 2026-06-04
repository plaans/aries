use aries::lang::Var;
use aries::lang::expr::eq_mul;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Multiplication constraint.
///
/// `a = b * c`
#[derive(Debug)]
pub struct Mul {
    a: Var,
    b: Var,
    c: Var,
}

impl Mul {
    pub fn new(a: Var, b: Var, c: Var) -> Self {
        Self { a, b, c }
    }

    pub fn a(&self) -> &Var {
        &self.a
    }

    pub fn b(&self) -> &Var {
        &self.b
    }

    pub fn c(&self) -> &Var {
        &self.c
    }
}

impl<Lbl: Label> Post<Lbl> for Mul {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce(eq_mul(self.a, self.b, self.c), []);
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_2;
    use crate::aries::constraint::test::basic_int_model_3;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, z) = basic_int_model_3();

        let mul = Mul::new(x, y, z);
        mul.post(&mut model);

        let verify = |[x, y, z]: [IntCst; 3]| x == y * z;

        verify_all([x, y, z], model, verify);
    }

    #[test]
    fn square() {
        let (mut model, x, y) = basic_int_model_2();

        let mul = Mul::new(x, y, y);
        mul.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x == y * y;

        verify_all([x, y], model, verify);
    }

    #[test]
    fn product_is_factor() {
        let (mut model, x, y) = basic_int_model_2();

        let mul = Mul::new(x, y, x);
        mul.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x == y * x;

        verify_all([x, y], model, verify);
    }
}
