use aries::model::extensions::AssignmentExt;
use aries::model::lang::max::EqMax;
use aries::model::lang::IAtom;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::Post;

/// Represent the constraint:
/// `b = abs(a)`
///
/// Endoded as `b = max(a,-a)`
pub struct Abs {
    a: IVar,
    b: IVar,
}

impl Abs {
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

impl<Lbl: Label> Post<Lbl> for Abs {
    fn post(&self, model: &mut Model<Lbl>) {
        let lb = model.lower_bound(self.a);
        let ub = model.upper_bound(self.b);
        let minus_a = model.state.new_var(-ub, -lb);
        let minus_a = IVar::new(minus_a);
        let minus_a = IAtom::new(minus_a, 0);
        let plus_a = IAtom::new(self.a, 0);
        let eq_max = EqMax::new(self.b, [plus_a, minus_a]);
        model.enforce(eq_max, []);
        todo!("work in progresss...");
    }
}

#[cfg(test)]
mod tests {
    use aries::solver::Solver;

    use super::*;

    fn _get_solutions(
        a: IVar,
        b: IVar,
        model: Model<String>,
    ) -> Vec<(i32, i32)> {
        let mut solver = Solver::new(model);
        solver
            .enumerate(&[a.into(), b.into()])
            .unwrap()
            .iter()
            .map(|v| (v[0], v[1]))
            .collect()
    }

    // #[test]
    fn _b_from_a() {
        let mut model: Model<String> = Model::new();

        let a = model.new_ivar(-3, -3, "a".to_string());
        let b = model.new_ivar(-5, 5, "b".to_string());

        let abs = Abs::new(a, b);
        abs.post(&mut model);

        let solutions = _get_solutions(a, b, model);

        assert_eq!(solutions, vec![(-3, 3)]);
    }
}
