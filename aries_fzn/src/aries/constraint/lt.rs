use aries::model::lang::expr::lt;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Less than constraint.
///
/// `a < b`
#[derive(Debug)]
pub struct Lt {
    a: IVar,
    b: IVar,
}

impl Lt {
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

impl<Lbl: Label> Post<Lbl> for Lt {
    fn post(&self, model: &mut Model<Lbl>) {
        model.enforce(lt(self.a, self.b), []);
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_2;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y) = basic_int_model_2();

        let lt = Lt::new(x, y);
        lt.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x < y;

        verify_all([x, y], model, verify);
    }
}
