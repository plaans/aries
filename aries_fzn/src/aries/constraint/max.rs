use aries_solver::lang::Var;
use aries_solver::model::Label;
use aries_solver::model::Model;
use aries_solver::prelude::eq_max;

use crate::aries::Post;

/// Maximum constraint.
///
/// `x = max(v[i])`
/// where `v[i]` are variables.
#[derive(Debug)]
pub struct Max {
    items: Vec<Var>,
    var: Var,
}

impl Max {
    pub fn new(items: Vec<Var>, var: Var) -> Self {
        Self { items, var }
    }

    pub fn items(&self) -> &Vec<Var> {
        &self.items
    }

    pub fn var(&self) -> &Var {
        &self.var
    }
}

impl<Lbl: Label> Post<Lbl> for Max {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce(eq_max(self.var, self.items.clone()));
    }
}

#[cfg(test)]
mod tests {
    use aries_solver::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_3;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, z) = basic_int_model_3();

        let max = Max::new(vec![x, y], z);
        max.post(&mut model);

        let verify = |[x, y, z]: [IntCst; 3]| x.max(y) == z;

        verify_all([x, y, z], model, verify);
    }
}
