use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::IVar;
use aries::model::lang::expr::eq;

use crate::aries::Post;

/// Reified not equal constraint.
///
/// `r <-> a != b`
#[derive(Debug)]
pub struct NeReif {
    a: IVar,
    b: IVar,
    r: BVar,
}

impl NeReif {
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

impl<Lbl: Label> Post<Lbl> for NeReif {
    fn post(&self, model: &mut Model<Lbl>) {
        model.bind(eq(self.a, self.b), self.r.false_lit());
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

        let eq_reif = NeReif::new(x, y, r);
        eq_reif.post(&mut model);

        let verify =
            |[x, y, r]: [IntCst; 3]| if x != y { r == 1 } else { r == 0 };

        verify_all([x, y, r.into()], model, verify);
    }
}
