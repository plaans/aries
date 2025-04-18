use aries::model::lang::BVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::ClauseReif;
use crate::aries::Post;

/// Reified or constraint.
///
/// `r <-> or(v[i])`
/// where `v[i]` are boolean variables.
#[derive(Debug)]
pub struct OrReif {
    variables: Vec<BVar>,
    r: BVar,
}

impl OrReif {
    pub fn new(variables: Vec<BVar>, r: BVar) -> Self {
        Self { variables, r }
    }

    pub fn variables(&self) -> &Vec<BVar> {
        &self.variables
    }

    pub fn r(&self) -> &BVar {
        &self.r
    }
}

impl<Lbl: Label> Post<Lbl> for OrReif {
    fn post(&self, model: &mut Model<Lbl>) {
        let clause_reif =
            ClauseReif::new(self.variables.clone(), Vec::new(), self.r);
        clause_reif.post(model);
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

        let or_reif = OrReif::new(vec![x, y], z);
        or_reif.post(&mut model);

        let verify = |[x, y, z]: [IntCst; 3]| (z == 1) == (x == 1 || y == 1);

        verify_all([x, y, z], model, verify);
    }
}
