use aries::model::Label;
use aries::model::Model;
use aries::model::lang::BVar;
use aries::model::lang::expr::and;

use crate::aries::Post;

/// Reified and constraint.
///
/// `b = and(v[i])`
/// where `v[i]` are boolean variables.
#[derive(Debug)]
pub struct AndReif {
    items: Vec<BVar>,
    var: BVar,
}

impl AndReif {
    pub fn new(items: Vec<BVar>, var: BVar) -> Self {
        Self { items, var }
    }

    pub fn items(&self) -> &Vec<BVar> {
        &self.items
    }

    pub fn var(&self) -> &BVar {
        &self.var
    }
}

impl<Lbl: Label> Post<Lbl> for AndReif {
    fn post(&self, model: &mut Model<Lbl>) {
        let a = and(self
            .items
            .iter()
            .cloned()
            .map(|v| v.true_lit())
            .collect::<Vec<_>>());
        model.bind(a, self.var.true_lit());
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

        let and_reif = AndReif::new(vec![x, y], z);
        and_reif.post(&mut model);

        // z = x and y iff z = min(x,y)
        let verify = |[x, y, z]: [IntCst; 3]| x.min(y) == z;

        verify_all([x, y, z], model, verify);
    }
}
