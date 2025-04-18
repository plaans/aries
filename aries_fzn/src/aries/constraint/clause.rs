use aries::core::Lit;
use aries::model::lang::expr::or;
use aries::model::lang::BVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Clause constraint.
///
/// `or(a[i]) \/ or(not b[i])`
/// where `a[i]` and `b[i]` are boolean variables.
#[derive(Debug)]
pub struct Clause {
    a: Vec<BVar>,
    b: Vec<BVar>,
}

impl Clause {
    pub fn new(a: Vec<BVar>, b: Vec<BVar>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &Vec<BVar> {
        &self.a
    }

    pub fn b(&self) -> &Vec<BVar> {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for Clause {
    fn post(&self, model: &mut Model<Lbl>) {
        let literals_a = self.a.iter().map(|v| v.true_lit());
        let literals_b = self.b.iter().map(|v| v.false_lit());
        let literals: Vec<Lit> = literals_a.chain(literals_b).collect();
        model.enforce(or(literals), []);
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

        let clause = Clause::new(vec![x, y], vec![z]);
        clause.post(&mut model);

        let verify = |[x, y, z]: [IntCst; 3]| x == 1 || y == 1 || z == 0;

        verify_all([x, y, z], model, verify);
    }
}
