use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::IVar;
use aries::model::lang::expr::lt;

use crate::aries::Post;

/// Reified equality constraint.
///
/// `r <-> a <= b`
#[derive(Debug)]
pub struct LtReif {
    a: IVar,
    b: IVar,
    r: BVar,
}

impl LtReif {
    pub fn new(a: IVar, b: IVar, r: BVar) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &IVar {
        &self.a
    }

    pub fn b(&self) -> &IVar {
        &self.b
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for LtReif {
    fn post(&self, model: &mut Model<Lbl>) {
        model.bind(lt(self.a, self.b), self.r.true_lit());
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

        let lt_reif = LtReif::new(x, y, r);
        lt_reif.post(&mut model);

        let verify = |[x, y, r]: [IntCst; 3]| (r == 1) == (x < y);

        verify_all([x, y, r.into()], model, verify);
    }
}
