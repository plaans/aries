use aries::model::lang::max::EqMin;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Represent the constraint:
/// `v = min(v_i)`
pub struct Min {
    items: Vec<IVar>,
    var: IVar,
}

impl Min {
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

impl<Lbl: Label> Post<Lbl> for Min {
    fn post(&self, model: &mut Model<Lbl>) {
        let eq_max = EqMin::new(self.var, self.items.clone());
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

        let max = Min::new(vec![x, y], z);
        max.post(&mut model);

        let verify = |x: IntCst, y, z| x.min(y) == z;

        verify_all_3(x, y, z, model, verify);
    }
}
