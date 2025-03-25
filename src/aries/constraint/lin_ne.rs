use aries::core::IntCst;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::lang::IVar;
use aries::model::Label;
use aries::model::Model;

use crate::aries::constraint::LinEq;
use crate::aries::constraint::Ne;
use crate::aries::Post;

/// Represent the constraint:
/// `sum(v_i * c_i) != b`
///
/// where `v_i` are variables, b and `c_i` constants
pub struct LinNe {
    sum: Vec<NFLinearSumItem>,
    b: IntCst,
}

impl LinNe {
    pub fn new(sum: Vec<NFLinearSumItem>, b: IntCst) -> Self {
        Self { sum, b }
    }

    pub fn sum(&self) -> &Vec<NFLinearSumItem> {
        &self.sum
    }

    pub fn b(&self) -> &IntCst {
        &self.b
    }

    /// Return true if it can be viewed as a basic not equal constraint.
    ///
    /// It is the case iff two coeffs are \[-1, 1\] or \[1, -1\].
    fn is_ne(&self) -> bool {
        if self.sum.len() != 2 {
            return false;
        }
        let f0 = self.sum[0].factor;
        let f1 = self.sum[1].factor;
        let (f0, f1) = (f0.min(f1), f0.max(f1));
        (f0, f1) == (-1, 1)
    }
}

impl<Lbl: Label> Post<Lbl> for LinNe {
    fn post(&self, model: &mut Model<Lbl>) {
        if self.is_ne() {
            let ne = Ne::new(self.sum[0].var, self.sum[1].var);
            ne.post(model);
            return;
        }

        // Compute the sum bounds
        let vlb = |v| model.state.lb(v);
        let vub = |v| model.state.ub(v);
        let ilb = |i: &NFLinearSumItem| {
            i.factor * if i.factor > 0 { vlb(i.var) } else { vub(i.var) }
        };
        let iub = |i: &NFLinearSumItem| {
            i.factor * if i.factor > 0 { vub(i.var) } else { vlb(i.var) }
        };

        let sum_lb = self.sum.iter().map(ilb).sum();
        let sum_ub = self.sum.iter().map(iub).sum();

        dbg!(&self.sum);
        dbg!(&sum_lb);
        dbg!(&sum_ub);

        let var_sum = model.state.new_var(sum_lb, sum_ub);

        let mut sum = self.sum.clone();
        sum.push(NFLinearSumItem {
            var: var_sum,
            factor: -1,
        });

        let lin_eq = LinEq::new(sum, 0);
        let ne = Ne::new(IVar::new(var_sum), self.b);
        lin_eq.post(model);
        ne.post(model);
    }
}

#[cfg(test)]
mod tests {
    use crate::aries::constraint::test::basic_lin_model;
    use crate::aries::constraint::test::verify_all_2;

    use super::*;

    #[test]
    fn basic() {
        let (mut model, sum, x, y, c_x, c_y, b) = basic_lin_model();

        let lin_ne = LinNe::new(sum, b);
        lin_ne.post(&mut model);

        let verify = |x, y| x * c_x + y * c_y != b;

        verify_all_2(x, y, model, verify);
    }

    #[test]
    fn is_ne() {
        let mut model: Model<&str> = Model::new();

        let x = model.new_ivar(1, 3, "x");
        let y = model.new_ivar(1, 3, "y");

        let sum = vec![
            NFLinearSumItem {
                var: x.into(),
                factor: 1,
            },
            NFLinearSumItem {
                var: y.into(),
                factor: -1,
            },
        ];

        // x - y != 0
        let lin_ne = LinNe::new(sum, 0);

        assert!(lin_ne.is_ne());
    }
}
