use aries::model::Label;
use aries::model::Model;
use aries::model::lang::IAtom;
use aries::model::lang::expr::neq;

use crate::aries::Post;

/// Not equal constraint.
///
/// `a != b`
#[derive(Debug)]
pub struct Ne {
    a: IAtom,
    b: IAtom,
}

impl Ne {
    pub fn new(a: impl Into<IAtom>, b: impl Into<IAtom>) -> Self {
        let a = a.into();
        let b = b.into();
        Self { a, b }
    }

    pub fn a(&self) -> &IAtom {
        &self.a
    }

    pub fn b(&self) -> &IAtom {
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
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_2;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y) = basic_int_model_2();

        let ne = Ne::new(x, y);
        ne.post(&mut model);

        let verify = |[x, y]: [IntCst; 2]| x != y;

        verify_all([x, y], model, verify);
    }
}
