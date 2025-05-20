use aries::core::Lit;
use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::expr::or;

use crate::aries::Post;

/// Reified clause constraint.
///
/// `r <-> or(a[i]) \/ or(not b[i])`
/// where `r`, `a[i]` and `b[i]` are boolean variables.
#[derive(Debug)]
pub struct ClauseReif {
    a: Vec<BVar>,
    b: Vec<BVar>,
    r: BVar,
}

impl ClauseReif {
    pub fn new(a: Vec<BVar>, b: Vec<BVar>, r: BVar) -> Self {
        Self { a, b, r }
    }

    pub fn a(&self) -> &Vec<BVar> {
        &self.a
    }

    pub fn b(&self) -> &Vec<BVar> {
        &self.b
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for ClauseReif {
    fn post(&self, model: &mut Model<Lbl>) {
        let literals_a = self.a.iter().map(|v| v.true_lit());
        let literals_b = self.b.iter().map(|v| v.false_lit());
        let literals: Vec<Lit> = literals_a.chain(literals_b).collect();
        model.bind(or(literals), self.r.true_lit());
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_bool_model_3;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, z) = basic_bool_model_3();

        let clause_reif = ClauseReif::new(vec![x], vec![y], z);
        clause_reif.post(&mut model);

        let verify = |[x, y, z]: [IntCst; 3]| (z == 1) == (x == 1 || y == 0);

        verify_all([x, y, z], model, verify);
    }
}
