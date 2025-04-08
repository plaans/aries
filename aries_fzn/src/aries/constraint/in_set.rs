use aries::core::IntCst;
use aries::model::lang::expr::or;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// In set constraint.
///
/// `x in {c[i]}`
/// where `c[i]` are constants.
#[derive(Debug)]
pub struct InSet {
    var: IVar,
    constants: Vec<IntCst>,
}

impl InSet {
    /// Create a new InSet constraint.
    ///
    /// It assumes the constants are sorted.
    pub fn new(var: IVar, constants: Vec<IntCst>) -> Self {
        debug_assert!(constants.is_sorted());
        Self { var, constants }
    }

    pub fn var(&self) -> &IVar {
        &self.var
    }

    pub fn constants(&self) -> &Vec<IntCst> {
        &self.constants
    }

    /// Return holes of the set.
    fn holes(&self) -> Vec<(IntCst, IntCst)> {
        let iter_lower = self.constants().iter().copied();
        let iter_upper = self.constants().iter().copied().skip(1);
        iter_lower
            .zip(iter_upper)
            .filter(|(l, u)| {
                dbg!(&(l, u));
                *u - *l > 1
            })
            .collect()
    }
}

impl<Lbl: Label> Post<Lbl> for InSet {
    fn post(&self, model: &mut Model<Lbl>) {
        // var >= min(constants)
        if let Some(lower) = self.constants.first() {
            model.enforce(self.var.geq(*lower), []);
        }

        // var <= max(constants)
        if let Some(upper) = self.constants.last() {
            model.enforce(self.var.leq(*upper), []);
        }

        // Forbid the variable to take any value in a hole
        let holes = self.holes();
        for (l, u) in holes {
            model.enforce(or([self.var.leq(l), self.var.geq(u)]), []);
        }
    }
}

#[cfg(test)]
mod tests {
    use aries::core::IntCst;
    use aries::core::VarRef;

    use crate::aries::constraint::test::basic_int_model_1;
    use crate::aries::constraint::test::verify_all_1;

    use super::*;

    #[test]
    fn holes() {
        let x = IVar::new(VarRef::from_u32(2));
        let set = vec![0, 2, 3, 6];
        let in_set = InSet::new(x, set);
        assert_eq!(in_set.holes(), vec![(0, 2), (3, 6)]);
    }

    #[test]
    fn basic() {
        let (mut model, x) = basic_int_model_1();

        let set = vec![0, 2, 3, 6];

        let in_set = InSet::new(x, set.clone());
        in_set.post(&mut model);

        let verify = |x: IntCst| set.contains(&x);

        verify_all_1(x, model, verify);
    }
}
