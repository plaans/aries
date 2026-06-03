use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::Var;
use aries::model::lang::expr::leq;

use crate::aries::Post;

/// Reified less or equal constraint.
///
/// `r <-> a <= b`
#[derive(Debug)]
pub struct LeReif {
    a: Var,
    b: Var,
    r: BVar,
}

impl LeReif {
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

impl<Lbl: Label> Post<Lbl> for LeReif {
    fn post(&self, model: &mut Model<Lbl>) {
        model.bind(leq(self.a, self.b), self.r.true_lit());
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_reif_model;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, r) = basic_reif_model();

        let eq_reif = LeReif::new(x, y, r);
        eq_reif.post(&mut model);

        let verify = |[x, y, r]: [IntCst; 3]| (r == 1) == (x <= y);

        verify_all([x, y, r.into()], model, verify);
    }
}
