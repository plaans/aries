use aries_solver::lang::VarCst;
use aries_solver::lang::expr::neq;
use aries_solver::model::Label;
use aries_solver::model::Model;

use crate::aries::Post;

/// Not equal constraint.
///
/// `a != b`
#[derive(Debug)]
pub struct Ne {
    a: VarCst,
    b: VarCst,
}

impl Ne {
    pub fn new(a: impl Into<VarCst>, b: impl Into<VarCst>) -> Self {
        let a = a.into();
        let b = b.into();
        Self { a, b }
    }

    pub fn a(&self) -> &VarCst {
        &self.a
    }

    pub fn b(&self) -> &VarCst {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for Ne {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce(neq(self.a, self.b));
    }
}

#[cfg(test)]
mod tests {
    use aries_solver::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_2;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y) = basic_int_model_2();

        let ne = Ne::new(x, y);
        ne.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x != y;

        verify_all([x, y], model, verify);
    }
}
