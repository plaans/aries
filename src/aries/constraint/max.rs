use aries::model::lang::max::EqMax;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Maximum constraint.
///
/// `x = max(v[i])`
/// where `v[i]` are variables.
#[derive(Debug)]
pub struct Max {
    items: Vec<IVar>,
    var: IVar,
}

impl Max {
    pub fn new(items: Vec<IVar>, var: IVar) -> Self {
        Self { items, var }
    }

    pub fn items(&self) -> &Vec<IVar> {
        &self.items
    }

    pub fn var(&self) -> &IVar {
        &self.var
    }
}

impl<Lbl: Label> Post<Lbl> for Max {
    fn post(&self, model: &mut Model<Lbl>) {
        let eq_max = EqMax::new(self.var, self.items.clone());
        model.enforce(eq_max, []);
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_3;
    use crate::aries::constraint::test::verify_all_3;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, z) = basic_int_model_3();

        let max = Max::new(vec![x, y], z);
        max.post(&mut model);

        let verify = |x: IntCst, y, z| x.max(y) == z;

        verify_all_3(x, y, z, model, verify);
    }
}
