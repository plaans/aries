use aries::core::IntCst;
use aries::model::lang::expr::eq;
use aries::model::lang::expr::geq;
use aries::model::lang::expr::implies;
use aries::model::lang::expr::lt;
use aries::model::lang::BVar;
use aries::model::lang::IAtom;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Element in array constraint.
///
/// `b = a[i]` where
/// `a[i]` are integer atoms,
/// `b` and `i` are integer variables.
#[derive(Debug)]
pub struct ArrayElement {
    a: Vec<IAtom>,
    b: IAtom,
    i: IAtom,
}

impl ArrayElement {
    pub fn new(
        a: Vec<IAtom>,
        b: impl Into<IAtom>,
        i: impl Into<IAtom>,
    ) -> Self {
        let b = b.into();
        let i = i.into();
        Self { a, b, i }
    }

    pub fn a(&self) -> &Vec<IAtom> {
        &self.a
    }

    pub fn b(&self) -> &IAtom {
        &self.b
    }

    pub fn i(&self) -> &IAtom {
        &self.i
    }
}

impl<Lbl: Label> Post<Lbl> for ArrayElement {
    fn post(&self, model: &mut Model<Lbl>) {
        // 0 <= i < len(a)
        model.enforce(geq(self.i, 0), []);
        model.enforce(lt(self.i, self.a.len() as IntCst), []);

        // i = j -> b = a[j]
        for (j, a_j) in self.a.iter().enumerate() {
            let i_eq_j = BVar::new(model.state.new_var(0, 1));
            let b_eq_a_j = BVar::new(model.state.new_var(0, 1));
            model.bind(eq(self.i, j as IntCst), i_eq_j.true_lit());
            model.bind(eq(self.b, *a_j), b_eq_a_j.true_lit());
            model.enforce(implies(i_eq_j, b_eq_a_j), []);
        }
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;

    use crate::aries::constraint::test::basic_int_model_3;
    use crate::aries::constraint::test::verify_all;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, x, y, z) = basic_int_model_3();

        let index_x = 2;

        let values = vec![1, 5, 0, 2, -1];
        let mut a: Vec<IAtom> =
            values.iter().cloned().map(|e| e.into()).collect();
        a[index_x] = x.into();

        let array_element = ArrayElement::new(a, y, z);
        array_element.post(&mut model);

        let verify = |[x, y, z]: [IntCst; 3]| {
            if z < 0 || z >= values.len() as IntCst {
                return false;
            }
            let z: usize = z.try_into().unwrap();
            if z == index_x {
                y == x
            } else {
                y == values[z]
            }
        };

        verify_all([x, y, z], model, verify);
    }
}
