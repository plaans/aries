use std::collections::BTreeMap;
use std::fmt::Display;

use crate::legacy::*;
use aries::core::state::Evaluable;
use aries::model::lang::linear::*;
use aries::{prelude::*, reif::ReifExpr};
use num_integer::lcm;
/* ========================================================================== */
/*                                  LinearLeq                                 */
/* ========================================================================== */

pub type QCst = num_rational::Ratio<IntCst>;

#[derive(Clone)]
pub struct LinearLeq {
    sum: LinearSum,
    ub: IntCst,
}

impl std::fmt::Display for LinearLeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <= {}", self.sum, self.ub)
    }
}

impl std::fmt::Debug for LinearLeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl LinearLeq {
    pub fn new(sum: LinearSum, ub: IntCst) -> LinearLeq {
        LinearLeq { sum, ub }
    }
}

// TODO: this is very suboptimal and misses many potential optimizations (e.g. 0 <= 0 should yield Lit::TRUE)
impl From<LinearLeq> for ReifExpr {
    fn from(value: LinearLeq) -> Self {
        let mut vars = BTreeMap::new();
        for e in &value.sum.terms {
            let var = e.var;
            let key = var;
            vars.entry(key)
                .and_modify(|factor| *factor += e.factor)
                .or_insert(e.factor);
        }
        ReifExpr::LinearLeq(LinSum::new(
            value.sum.constant - value.ub,
            vars.iter().map(|(&var, &factor)| ScaledVar { var, factor }),
        ))
    }
}

/* ========================================================================== */
/*                                 LinearTerm                                 */
/* ========================================================================== */

/// A linear term of the form `a/b * X` where:
///  - `a` and `b` are integer constants
///  - `X` is an integer variable.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LinearTerm {
    factor: IntCst,
    var: IVar,
    denom: IntCst,
}

impl Display for LinearTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.factor != 1 {
            if self.factor == -1 {
                write!(f, "-")?;
            } else {
                write!(f, "{}", self.factor)?;
            }
        }
        if self.factor.abs() != 1 && self.var != IVar::ONE {
            write!(f, "*")?;
        }
        if self.var != IVar::ONE {
            write!(f, "{:?}", self.var)?;
        } else if self.factor.abs() == 1 {
            write!(f, "1")?;
        }
        Ok(())
    }
}

impl LinearTerm {
    pub const fn new(factor: IntCst, var: IVar, denom: IntCst) -> LinearTerm {
        LinearTerm { factor, var, denom }
    }

    pub const fn int(factor: IntCst, var: IVar) -> LinearTerm {
        LinearTerm { factor, var, denom: 1 }
    }

    pub const fn rational(factor: IntCst, var: IVar, denom: IntCst) -> LinearTerm {
        LinearTerm { factor, var, denom }
    }

    pub const fn constant_int(value: IntCst) -> LinearTerm {
        LinearTerm {
            factor: value,
            var: IVar::ONE,
            denom: 1,
        }
    }

    pub const fn constant_rational(num: IntCst, denom: IntCst) -> LinearTerm {
        LinearTerm {
            factor: num,
            var: IVar::ONE,
            denom,
        }
    }

    pub fn denom(&self) -> IntCst {
        self.denom
    }

    pub fn factor(&self) -> IntCst {
        self.factor
    }

    pub fn var(&self) -> IVar {
        self.var
    }
}

impl From<IVar> for LinearTerm {
    fn from(var: IVar) -> Self {
        LinearTerm::int(1, var)
    }
}

impl From<IntCst> for LinearTerm {
    fn from(value: IntCst) -> Self {
        LinearTerm::constant_int(value)
    }
}

impl std::ops::Neg for LinearTerm {
    type Output = LinearTerm;

    fn neg(self) -> Self::Output {
        LinearTerm {
            factor: -self.factor,
            var: self.var,
            denom: self.denom,
        }
    }
}

/* ========================================================================== */
/*                                  LinearSum                                 */
/* ========================================================================== */

/// A linear sum of the form `a1/b * X1 + a2/b * X2 + ... + Y/b` where `ai`, `b` and `Y` are integer constants and `Xi` is a variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinearSum {
    /// Linear terms of sum, each of the form `ai / b * Xi`.
    /// Invariant: the denominator `b` of all elements of the sum must be the same as `self.denom`
    terms: Vec<LinearTerm>,
    constant: IntCst,
    /// Denominator of all elements of the linear sum.
    denom: IntCst,
}

impl std::fmt::Display for LinearSum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.terms.iter().enumerate() {
            if e.factor < 0 {
                if i != 0 {
                    write!(f, " - ")?;
                } else {
                    write!(f, "-")?;
                }
            } else if i > 0 {
                write!(f, " + ")?;
            }
            if e.factor.abs() != 1 {
                write!(f, "{}", e.factor.abs())?;
            }
            if e.factor.abs() != 1 && e.var != IVar::ONE {
                write!(f, "*")?;
            }
            if e.var != IVar::ONE {
                write!(f, "{:?}", e.var)?;
            } else if e.factor.abs() == 1 {
                write!(f, "1")?;
            }
        }
        if self.constant != 0 {
            if !self.terms.is_empty() {
                write!(f, " + ")?;
            }
            write!(f, "{}", self.constant)?;
        }
        Ok(())
    }
}

impl LinearSum {
    pub fn zero() -> LinearSum {
        LinearSum {
            terms: Vec::new(),
            constant: 0,
            denom: 1,
        }
    }

    pub fn constant_int(n: IntCst) -> LinearSum {
        LinearSum {
            terms: Vec::new(),
            constant: n,
            denom: 1,
        }
    }

    pub fn constant_rational(num: IntCst, denom: IntCst) -> LinearSum {
        Self {
            terms: vec![],
            constant: num,
            denom,
        }
    }

    pub fn of<T: Into<LinearSum> + Clone>(elements: Vec<T>) -> LinearSum {
        let mut res = LinearSum::zero();
        for e in elements {
            res += e.into()
        }
        res
    }

    fn set_denom(&mut self, new_denom: IntCst) {
        debug_assert_eq!(new_denom % self.denom, 0);
        let scaling_factor = new_denom / self.denom;
        if scaling_factor != 1 {
            for term in self.terms.as_mut_slice() {
                debug_assert_eq!(term.denom, self.denom);
                term.factor *= scaling_factor;
                term.denom = new_denom;
            }
            self.constant *= scaling_factor;
            self.denom = new_denom;
        }
    }

    fn add_term(&mut self, mut added: LinearTerm) {
        let new_denom = num_integer::lcm(self.denom, added.denom);
        self.set_denom(new_denom);
        added.factor *= new_denom / added.denom;
        added.denom = new_denom;
        self.terms.push(added);
    }

    fn add_rational(&mut self, num: IntCst, denom: IntCst) {
        let new_denom = lcm(self.denom, denom);
        self.set_denom(new_denom);
        let scaled_num = num * new_denom / denom;
        self.constant += scaled_num;
    }

    pub fn leq<T: Into<LinearSum>>(self, upper_bound: T) -> LinearLeq {
        LinearLeq::new(self - upper_bound, 0)
    }
    pub fn geq<T: Into<LinearSum>>(self, lower_bound: T) -> LinearLeq {
        (-self).leq(-lower_bound.into())
    }

    pub fn constant(&self) -> IntCst {
        self.constant
    }

    pub fn denom(&self) -> IntCst {
        self.denom
    }

    pub fn terms(&self) -> &[LinearTerm] {
        self.terms.as_ref()
    }

    /// Returns a new `LinearSum` without the terms with a null `factor` or the `variable` ZERO.
    /// The terms are grouped by (`variable`, `lit`) and the constant terms and grouped into the `constant`.
    pub fn simplify(&self) -> LinearSum {
        let mut term_map = BTreeMap::new();
        let mut constant = self.constant;
        for term in &self.terms {
            // By construction, all terms should have the same denom. Check it.
            debug_assert_eq!(term.denom, self.denom);

            // Group the terms by their `variable` and `lit` attribute.
            term_map
                .entry(term.var)
                .and_modify(|f| *f += term.factor)
                .or_insert(term.factor);

            // Group the constant terms into the constant.
            if term.var == IVar::ONE {
                constant += term.factor;
            }
        }

        // Filter the null `factor`, the `variable` ZERO, and the constant terms.
        LinearSum {
            constant,
            denom: self.denom,
            terms: term_map
                .into_iter()
                .filter(|(v, f)| *f != 0 && *v != IVar::ZERO)
                .filter(|(v, _)| *v != IVar::ONE) // Has been grouped into the constant
                .map(|(v, f)| LinearTerm {
                    factor: f,
                    var: v,
                    denom: self.denom,
                })
                .collect(),
        }
    }
}

impl From<LinearTerm> for LinearSum {
    fn from(term: LinearTerm) -> Self {
        LinearSum {
            terms: vec![term],
            constant: 0,
            denom: term.denom,
        }
    }
}
impl From<IntCst> for LinearSum {
    fn from(constant: IntCst) -> Self {
        LinearSum::constant_int(constant)
    }
}

impl From<ScaledVar> for LinearSum {
    fn from(value: ScaledVar) -> Self {
        LinearSum::from(LinearTerm::int(value.factor, value.var))
    }
}

impl From<FAtom> for LinearSum {
    fn from(value: FAtom) -> Self {
        let mut sum = LinearSum {
            terms: vec![LinearTerm {
                factor: 1,
                var: value.num.var,
                denom: value.denom,
            }],
            constant: 0,
            denom: value.denom,
        };
        sum += LinearTerm::constant_rational(value.num.shift, value.denom);
        sum
    }
}

impl From<IAtom> for LinearSum {
    fn from(value: IAtom) -> Self {
        let mut sum = LinearSum {
            terms: vec![LinearTerm {
                factor: 1,
                var: value.var,
                denom: 1,
            }],
            constant: 0,
            denom: 1,
        };
        sum += LinearTerm::constant_int(value.shift);
        sum
    }
}

impl From<SignedVar> for LinearTerm {
    fn from(value: SignedVar) -> Self {
        LinearTerm {
            factor: if value.is_plus() { 1 } else { -1 },
            var: value.variable(),
            denom: 1,
        }
    }
}

impl From<SignedVar> for LinearSum {
    fn from(value: SignedVar) -> Self {
        LinearSum::from(LinearTerm::from(value))
    }
}

impl<T: Into<LinearSum>> std::ops::Add<T> for LinearSum {
    type Output = LinearSum;

    fn add(self, rhs: T) -> Self::Output {
        let mut new = self.clone();
        new += rhs;
        new
    }
}

impl<T: Into<LinearSum>> std::ops::Sub<T> for LinearSum {
    type Output = LinearSum;

    fn sub(self, rhs: T) -> Self::Output {
        self + (-rhs.into())
    }
}

impl<T: Into<IntCst>> std::ops::Mul<T> for LinearSum {
    type Output = LinearSum;

    fn mul(self, rhs: T) -> Self::Output {
        let mut new = self.clone();
        new *= rhs;
        new
    }
}

impl<T: Into<LinearSum>> std::ops::AddAssign<T> for LinearSum {
    fn add_assign(&mut self, rhs: T) {
        let rhs: LinearSum = rhs.into();
        for term in rhs.terms {
            self.add_term(term);
        }
        self.add_rational(rhs.constant, rhs.denom);
    }
}

impl<T: Into<LinearSum>> std::ops::SubAssign<T> for LinearSum {
    fn sub_assign(&mut self, rhs: T) {
        let sum: LinearSum = -rhs.into();
        *self += sum;
    }
}

impl<T: Into<IntCst>> std::ops::MulAssign<T> for LinearSum {
    fn mul_assign(&mut self, rhs: T) {
        let rhs = rhs.into();
        self.constant *= rhs;
        for term in self.terms.iter_mut() {
            term.factor *= rhs;
        }
    }
}

impl std::ops::Neg for LinearSum {
    type Output = LinearSum;

    fn neg(mut self) -> Self::Output {
        for e in &mut self.terms {
            *e = -(*e)
        }
        self.constant = -self.constant;
        self
    }
}

impl TryFrom<LinearSum> for IAtom {
    type Error = ();

    fn try_from(value: LinearSum) -> Result<Self, Self::Error> {
        let value = value.simplify();
        if value.denom != 1 {
            return Err(());
        }
        let var = if value.terms.is_empty() {
            IVar::ZERO
        } else if value.terms.len() == 1 {
            let term = value.terms[0];
            debug_assert_eq!(term.denom, 1);
            if term.factor() == 1 {
                term.var
            } else {
                return Err(());
            }
        } else {
            return Err(());
        };
        Ok(var + value.constant)
    }
}

use aries::transitive_conversion;

transitive_conversion!(LinearSum, LinearTerm, IVar);

impl Evaluable for LinearSum {
    type Value = QCst;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        let mut sum = QCst::new(self.constant, self.denom);
        for term in &self.terms {
            // add the contribution of each term, BUT
            // we shortcircuit and return None if the term is absent
            sum += term.evaluate(solution)?;
        }
        Some(sum)
    }
}

impl Evaluable for LinearTerm {
    type Value = QCst;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        let var_value = self.var.evaluate(solution)?;
        Some(QCst::new(self.factor, self.denom) * QCst::from_integer(var_value))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    /* ========================== LinearTerm Tests ========================== */

    #[test]
    fn test_term_new() {
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(15);
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                        let term = LinearTerm::new(ff * f, v, d);
                        assert_eq!(term.factor, ff * f);
                        assert_eq!(term.var, v);
                        assert_eq!(term.denom, d);
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_int() {
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(15);
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [var1, var2] {
                    let term = LinearTerm::int(ff * f, v);
                    assert_eq!(term.factor, ff * f);
                    assert_eq!(term.var, v);
                    assert_eq!(term.denom, 1);
                }
            }
        }
    }

    #[test]
    fn test_term_rational() {
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(15);
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [var1, var2] {
                    for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                        let term = LinearTerm::rational(ff * f, v, d);
                        assert_eq!(term.factor, ff * f);
                        assert_eq!(term.var, v);
                        assert_eq!(term.denom, d);
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_constant_int() {
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                let term = LinearTerm::constant_int(ff * f);
                assert_eq!(term.factor, ff * f);
                assert_eq!(term.var, IVar::ONE);
                assert_eq!(term.denom, 1);
            }
        }
    }

    #[test]
    fn test_term_constant_rational() {
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                    let term = LinearTerm::constant_rational(ff * f, d);
                    assert_eq!(term.factor, ff * f);
                    assert_eq!(term.var, IVar::ONE);
                    assert_eq!(term.denom, d);
                }
            }
        }
    }

    #[test]
    fn test_term_from_ivar() {
        let var0 = IVar::ZERO;
        let var1 = IVar::ONE;
        let var2 = Var::from_u32(5);
        let var3 = Var::from_u32(15);
        for v in [var0, var1, var2, var3] {
            let term = LinearTerm::from(v);
            let expected = LinearTerm::int(1, v);
            assert_eq!(term, expected);
        }
    }

    #[test]
    fn test_term_from_int_cst() {
        for i in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            let term = LinearTerm::from(i);
            let expected = LinearTerm::constant_int(i);
            assert_eq!(term, expected);
        }
    }

    #[test]
    fn test_term_eq() {
        let mut terms: Vec<LinearTerm> = vec![];
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(15);
        for f in [1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                        let term = LinearTerm::new(ff * f, v, d);
                        terms.push(term);
                    }
                }
            }
        }

        for (i, t1) in terms.iter().enumerate() {
            for (j, t2) in terms.iter().enumerate() {
                assert_eq!(i == j, t1 == t2);
            }
        }
    }

    #[test]
    fn test_term_neg() {
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(15);
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                        let term = -LinearTerm::new(ff * f, v, d);
                        let expected = LinearTerm::new(-ff * f, v, d);
                        assert_eq!(term, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_getters() {
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(15);
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                        let term = LinearTerm::new(ff * f, v, d);
                        assert_eq!(term.factor, term.factor());
                        assert_eq!(term.var, term.var());
                        assert_eq!(term.denom, term.denom());
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_display() {
        let var = Var::from_u32(5);
        // Constant terms
        assert_eq!(format!("{}", LinearTerm::constant_int(1)), "1");
        assert_eq!(format!("{}", LinearTerm::constant_int(-1)), "-1");
        assert_eq!(format!("{}", LinearTerm::constant_int(5)), "5");
        assert_eq!(format!("{}", LinearTerm::constant_int(-5)), "-5");
        assert_eq!(format!("{}", LinearTerm::constant_rational(1, 10)), "1");
        assert_eq!(format!("{}", LinearTerm::constant_rational(-1, 10)), "-1");
        assert_eq!(format!("{}", LinearTerm::constant_rational(5, 10)), "5");
        assert_eq!(format!("{}", LinearTerm::constant_rational(-5, 10)), "-5");
        // Pseudo-constant terms
        assert_eq!(format!("{}", LinearTerm::int(1, IVar::ONE)), "1");
        assert_eq!(format!("{}", LinearTerm::int(-1, IVar::ONE)), "-1");
        assert_eq!(format!("{}", LinearTerm::int(5, IVar::ONE)), "5");
        assert_eq!(format!("{}", LinearTerm::int(-5, IVar::ONE)), "-5");
        assert_eq!(format!("{}", LinearTerm::rational(1, IVar::ONE, 10)), "1");
        assert_eq!(format!("{}", LinearTerm::rational(-1, IVar::ONE, 10)), "-1");
        assert_eq!(format!("{}", LinearTerm::rational(5, IVar::ONE, 10)), "5");
        assert_eq!(format!("{}", LinearTerm::rational(-5, IVar::ONE, 10)), "-5");
        // Variable terms
        assert_eq!(format!("{}", LinearTerm::int(1, var)), "var5");
        assert_eq!(format!("{}", LinearTerm::int(-1, var)), "-var5");
        assert_eq!(format!("{}", LinearTerm::int(5, var)), "5*var5");
        assert_eq!(format!("{}", LinearTerm::int(-5, var)), "-5*var5");
        assert_eq!(format!("{}", LinearTerm::rational(1, var, 10)), "var5");
        assert_eq!(format!("{}", LinearTerm::rational(-1, var, 10)), "-var5");
        assert_eq!(format!("{}", LinearTerm::rational(5, var, 10)), "5*var5");
        assert_eq!(format!("{}", LinearTerm::rational(-5, var, 10)), "-5*var5");
    }

    /* =========================== LinearSum Tests ========================== */

    #[test]
    fn test_sum_zero() {
        let sum = LinearSum::zero();
        assert_eq!(sum.terms, vec![]);
        assert_eq!(sum.constant, 0);
        assert_eq!(sum.denom, 1);
    }

    #[test]
    fn test_sum_constant_int() {
        for n in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            let sum = LinearSum::constant_int(n);
            assert_eq!(sum.terms, vec![]);
            assert_eq!(sum.constant, n);
            assert_eq!(sum.denom, 1);
        }
    }

    #[test]
    fn test_sum_constant_rational() {
        for n in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                let sum = LinearSum::constant_rational(n, d);
                assert_eq!(sum.terms, vec![]);
                assert_eq!(sum.constant, n);
                assert_eq!(sum.denom, d);
            }
        }
    }

    #[test]
    fn test_sum_of_elements_same_denom() {
        let var = Var::from_u32(5);
        let terms = vec![LinearTerm::rational(1, var, 10), LinearTerm::constant_rational(5, 10)];
        let sum = LinearSum::of(terms.clone());
        assert_eq!(sum.constant, 0);
        assert_eq!(sum.denom, 10);
        assert_eq!(sum.terms, terms);
    }

    #[test]
    fn test_sum_of_elements_different_denom() {
        let terms = vec![
            LinearTerm::constant_rational(5, 28),
            LinearTerm::constant_rational(10, 77),
            LinearTerm::constant_rational(-3, 77),
        ];

        let expected_terms = vec![
            LinearTerm::constant_rational(55, 308),
            LinearTerm::constant_rational(40, 308),
            LinearTerm::constant_rational(-12, 308),
        ];
        let sum = LinearSum::of(terms);
        assert_eq!(sum.constant, 0);
        assert_eq!(sum.denom, 308);
        assert_eq!(sum.terms, expected_terms);
    }

    #[test]
    fn test_sum_set_denom() {
        let terms = [
            LinearTerm::constant_rational(5, 28),
            LinearTerm::constant_rational(10, 77),
        ];
        let expected_terms = vec![
            LinearTerm::constant_rational(55, 308),
            LinearTerm::constant_rational(40, 308),
        ];
        for (&t, e) in terms.iter().zip(expected_terms) {
            let mut sum = LinearSum::constant_int(3) + LinearSum::of(vec![t]);
            sum.set_denom(308);
            assert_eq!(sum.constant, 3 * 308);
            assert_eq!(sum.denom, 308);
            assert_eq!(sum.terms, vec![e]);
        }
    }

    #[test]
    fn test_sum_add_term() {
        let mut sum = LinearSum::constant_rational(3, 77);
        sum.add_term(LinearTerm::constant_rational(5, 28));
        assert_eq!(sum.constant, 12);
        assert_eq!(sum.denom, 308);
        assert_eq!(sum.terms, vec![LinearTerm::constant_rational(55, 308)]);
    }

    #[test]
    fn test_sum_add_rational() {
        let mut sum = LinearSum::of(vec![LinearTerm::constant_rational(5, 28)]);
        sum.add_rational(3, 77);
        assert_eq!(sum.constant, 12);
        assert_eq!(sum.denom, 308);
        assert_eq!(sum.terms, vec![LinearTerm::constant_rational(55, 308)]);
    }

    #[test]
    fn test_sum_leq() {
        for n in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                for u in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
                    let sum = LinearSum::constant_rational(n, d);
                    let leq = sum.clone().leq(u);
                    assert_eq!(leq.sum, sum - u);
                    assert_eq!(leq.ub, 0);
                }
            }
        }
    }

    #[test]
    fn test_sum_geq() {
        for n in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                for l in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
                    let sum = LinearSum::constant_rational(n, d);
                    let leq = sum.clone().geq(l);
                    assert_eq!(leq.sum, -(sum - l));
                    assert_eq!(leq.ub, 0);
                }
            }
        }
    }

    #[test]
    fn test_sum_getters() {
        // The values of the sum attributes are tested in other tests.
        // We are only checking that the getters return the current value.

        // Tests with different constants and denom
        for n in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                let sum = LinearSum::constant_rational(n, d);
                assert_eq!(sum.terms(), sum.terms);
                assert_eq!(sum.constant(), sum.constant);
                assert_eq!(sum.denom(), sum.denom);
            }
        }

        // Test with different terms
        let var = Var::from_u32(5);
        let terms = vec![LinearTerm::rational(1, var, 10), LinearTerm::constant_rational(5, 10)];
        let sum = LinearSum::of(terms.clone());
        assert_eq!(sum.constant(), sum.constant);
        assert_eq!(sum.denom(), sum.denom);
        assert_eq!(sum.terms(), sum.terms);
    }

    #[test]
    fn test_sum_simplify() {
        // Terms should be grouped by (lit, variable)
        // Terms with null `factor` or `variable` equals to Var::ZERO should be filtered
        // Terms with null `variable` and `literal` equals to Lit::TRUE  should be grouped into the constant
        let denom = 100;
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(6);

        let sum = LinearSum {
            constant: 5,
            denom,
            terms: vec![
                // Constant terms should be in the constant
                LinearTerm::new(10, IVar::ONE, denom),
                LinearTerm::new(15, IVar::ONE, denom),
                LinearTerm::new(20, IVar::ONE, denom),
                LinearTerm::new(25, IVar::ONE, denom),
                // Variable terms with zero variable, should be filtered
                LinearTerm::new(30, IVar::ZERO, denom),
                LinearTerm::new(35, IVar::ZERO, denom),
                LinearTerm::new(40, IVar::ZERO, denom),
                LinearTerm::new(45, IVar::ZERO, denom),
                // Variable terms with null factor, should be filtered
                LinearTerm::new(0, var1, denom),
                LinearTerm::new(0, var2, denom),
                LinearTerm::new(0, var1, denom),
                LinearTerm::new(0, var1, denom),
                // Other variable terms no specificities, should be grouped by lit
                LinearTerm::new(50, var2, denom),
                LinearTerm::new(55, var1, denom),
                LinearTerm::new(60, var2, denom),
                LinearTerm::new(65, var2, denom),
            ],
        }
        .simplify();

        assert_eq!(sum.constant, 75);
        assert_eq!(sum.denom, 100);

        // Terms could have been reorganized
        let expected_terms = [
            // Other variable terms no specificities, should be grouped by lit
            LinearTerm::new(55, var1, denom),
            LinearTerm::new(175, var2, denom),
        ];
        assert_eq!(sum.terms.len(), expected_terms.len());
        for term in sum.terms {
            assert!(expected_terms.contains(&term));
        }
    }

    #[test]
    fn test_sum_from_linear_term() {
        let terms = vec![
            LinearTerm::constant_rational(5, 28),
            LinearTerm::constant_rational(10, 77),
            LinearTerm::constant_rational(-3, 77),
        ];
        for t in terms {
            let sum = LinearSum::from(t);
            assert_eq!(sum.constant, 0);
            assert_eq!(sum.denom, t.denom);
            assert_eq!(sum.terms, vec![t]);
        }
    }

    #[test]
    fn test_sum_from_int_cst() {
        for i in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            let sum = LinearSum::from(i);
            assert_eq!(sum.constant, i);
            assert_eq!(sum.denom, 1);
            assert_eq!(sum.terms, vec![]);
        }
    }

    #[test]
    fn test_sum_from_fatom() {
        let var0 = IVar::ZERO;
        let var1 = IVar::ONE;
        let var2 = Var::from_u32(5);
        let var3 = Var::from_u32(15);
        for s in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                for v in [var0, var1, var2, var3] {
                    let fa = FAtom::new(IAtom::new(v, s), d);
                    let sum = LinearSum::from(fa);
                    assert_eq!(sum.constant, 0);
                    assert_eq!(sum.denom, d);
                    assert_eq!(
                        sum.terms,
                        vec![LinearTerm::new(1, v, d), LinearTerm::constant_rational(s, d),]
                    );
                }
            }
        }
    }

    #[test]
    fn test_sum_from_iatom() {
        let var0 = IVar::ZERO;
        let var1 = IVar::ONE;
        let var2 = Var::from_u32(5);
        let var3 = Var::from_u32(15);
        for s in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for v in [var0, var1, var2, var3] {
                let ia = IAtom::new(v, s);
                let sum = LinearSum::from(ia);
                assert_eq!(sum.constant, 0);
                assert_eq!(sum.denom, 1);
                assert_eq!(sum.terms, vec![LinearTerm::new(1, v, 1), LinearTerm::constant_int(s),]);
            }
        }
    }

    #[test]
    fn test_sum_add() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        let result = (s1 + s2).simplify();
        assert_eq!(result.constant, 95);
        assert_eq!(result.denom, 308);
        assert_eq!(result.terms, vec![]);
    }

    #[test]
    fn test_sum_mul() {
        let v = Var::from_u32(5);
        let s = LinearSum::of(vec![FAtom::new(IAtom::new(v, 5), 28)]);
        let result = (s * 3).simplify();
        assert_eq!(result.constant, 15);
        assert_eq!(result.denom, 28);
        assert_eq!(result.terms, vec![LinearTerm::new(3, v, 28)]);
    }

    #[test]
    fn test_sum_sub() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        let result = (s1 - s2).simplify();
        assert_eq!(result.constant, 15);
        assert_eq!(result.denom, 308);
        assert_eq!(result.terms, vec![]);
    }

    #[test]
    fn test_sum_add_assign() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        let mut result = s1.clone();
        result += s2;
        let result = result.simplify();
        assert_eq!(result.constant, 95);
        assert_eq!(result.denom, 308);
        assert_eq!(result.terms, vec![]);
    }

    #[test]
    fn test_sum_sub_assign() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        let mut result = s1.clone();
        result -= s2;
        let result = result.simplify();
        assert_eq!(result.constant, 15);
        assert_eq!(result.denom, 308);
        assert_eq!(result.terms, vec![]);
    }

    #[test]
    fn test_sum_mul_assign() {
        let v = Var::from_u32(5);
        let s = LinearSum::of(vec![FAtom::new(IAtom::new(v, 5), 28)]);
        let mut result = s.clone();
        result *= 3;
        let result = result.simplify();
        assert_eq!(result.constant, 15);
        assert_eq!(result.denom, 28);
        assert_eq!(result.terms, vec![LinearTerm::new(3, v, 28)]);
    }

    #[test]
    fn test_sum_neg() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        for s in [s1, s2] {
            let n = -s.clone();
            assert_eq!(n.constant, -s.constant);
            assert_eq!(n.denom, n.denom);
            for (&nt, &st) in n.terms.iter().zip(s.terms.iter()) {
                assert_eq!(nt, -st);
            }
        }
    }

    #[test]
    fn test_sum_display() {
        let var = Var::from_u32(5);
        // Simple addition
        let sum = LinearSum::of(vec![
            LinearTerm::rational(1, var, 10),
            LinearTerm::constant_rational(5, 10),
            LinearTerm::rational(5, var, 10),
            LinearTerm::constant_rational(1, 10),
        ]);
        assert_eq!(format!("{}", sum), "var5 + 5 + 5*var5 + 1");
        // Simple subtraction
        let sum = LinearSum::of(vec![
            LinearTerm::rational(1, var, 10),
            LinearTerm::constant_rational(-5, 10),
            LinearTerm::rational(-5, var, 10),
            LinearTerm::constant_rational(-1, 10),
        ]);
        assert_eq!(format!("{}", sum), "var5 - 5 - 5*var5 - 1");
        // Complete subtraction
        let sum = LinearSum::of(vec![
            LinearTerm::rational(-1, var, 10),
            LinearTerm::constant_rational(-5, 10),
        ]);
        assert_eq!(format!("{}", sum), "-var5 - 5");
    }

    /* ================================ Utils =============================== */

    #[test]
    fn test_lcm() {
        assert_eq!(lcm(30, 36), 180);
        assert_eq!(lcm(1, 10), 10);
        assert_eq!(lcm(33, 12), 132);
        assert_eq!(lcm(27, 48), 432);
        assert_eq!(lcm(17, 510), 510);
        assert_eq!(lcm(14, 18), 126);
        assert_eq!(lcm(39, 45), 585);
        assert_eq!(lcm(39, 130), 390);
        assert_eq!(lcm(28, 77), 308);
    }
}
