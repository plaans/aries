use aries::model::lang::expr::or;
use aries::model::lang::BVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Represent the constraint:
/// `v = or(v_i)`
pub struct OrReif {
    items: Vec<BVar>,
    var: BVar,
}

impl OrReif {
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

impl<Lbl: Label> Post<Lbl> for OrReif {
    fn post(&self, model: &mut Model<Lbl>) {
        let o = or(self
            .items
            .iter()
            .cloned()
            .map(|v| v.true_lit())
            .collect::<Vec<_>>());
        model.bind(o, self.var.true_lit());
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_bool_model_3;
    use crate::aries::constraint::test::verify_all_3;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, z) = basic_bool_model_3();

        let or_reif = OrReif::new(vec![x, y], z);
        or_reif.post(&mut model);

        // z = x or y iff z = max(x,y)
        let verify = |x: IntCst, y, z| x.max(y) == z;

        verify_all_3(x, y, z, model, verify);
    }
}
