use aries::model::lang::expr::neq;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Represent the constraint:
/// `a != b`
pub struct Ne {
    a: IVar,
    b: IVar,
}

impl Ne {
    pub fn new(a: IVar, b: IVar) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> &IVar {
        &self.a
    }

    pub fn b(&self) -> &IVar {
        &self.b
    }
}

impl<Lbl: Label> Post<Lbl> for Ne {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce(neq(self.a, self.b), []);
    }
}

#[cfg(test)]
mod tests {
    use crate::aries::constraint::test::basic_int_model_2;
    use crate::aries::constraint::test::verify_all_2;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y) = basic_int_model_2();

        let ne = Ne::new(x, y);
        ne.post(&mut model);

        let verify = |x, y| x != y;

        verify_all_2(x, y, model, verify);
    }
}
