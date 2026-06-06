use aries_solver::lang::BVar;
use aries_solver::lang::Var;
use aries_solver::lang::expr::eq;
use aries_solver::model::Label;
use aries_solver::model::Model;

use crate::aries::Post;

/// Half reified equality constraint.
///
/// `r -> a = b`
#[derive(Debug)]
pub struct EqHalf {
    a: Var,
    b: Var,
    r: BVar,
}

impl EqHalf {
    pub fn new(a: Var, b: Var, r: BVar) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Var {
        &self.a
    }

    pub fn b(&self) -> &Var {
        &self.b
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for EqHalf {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce_if(self.r.true_lit(), eq(self.a, self.b));
    }
}

#[cfg(test)]
mod tests {
    use aries_solver::core::IntCst;

    use crate::aries::constraint::test::basic_reif_model;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, r) = basic_reif_model();

        let eq_reif = EqHalf::new(x, y, r);
        eq_reif.post(&mut model);

        let verify = |[x, y, r]: [IntCst; 3]| (r == 0) || (x == y);

        verify_all([x, y, r.into()], model, verify);
    }
}
