use aries::model::Label;
use aries::model::Model;
use aries::model::lang::IVar;
use aries::model::lang::expr::eq;

use crate::aries::Post;

/// Equality constraint.
///
/// `a = b`
#[derive(Debug)]
pub struct Eq {
    a: IVar,
    b: IVar,
}

impl Eq {
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

impl<Lbl: Label> Post<Lbl> for Eq {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce(eq(self.a, self.b), []);
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

        let eq = Eq::new(x, y);
        eq.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x == y;

        verify_all([x, y], model, verify);
    }
}
