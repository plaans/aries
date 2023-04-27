use num_integer::lcm;

use crate::core::{IntCst, Lit, VarRef};
use crate::model::lang::{IAtom, IVar, ValidityScope};
use crate::reif::ReifExpr;
use std::collections::BTreeMap;

/// A linear term of the form `a/b * X` where `a` and `b` are constants and `X` is a variable.
#[derive(Copy, Clone, Debug)]
pub struct LinearTerm {
    factor: IntCst,
    var: IVar,
    /// If true, then this term should be interpreted as zero if the variable is absent.
    or_zero: bool,
    denom: IntCst,
}

impl LinearTerm {
    pub const fn new(factor: IntCst, var: IVar, or_zero: bool) -> LinearTerm {
        LinearTerm {
            factor,
            var,
            or_zero,
            denom: 1,
        }
    }

    pub fn or_zero(self) -> Self {
        LinearTerm {
            factor: self.factor,
            var: self.var,
            or_zero: true,
            denom: self.denom,
        }
    }

    pub fn denom(&self) -> IntCst {
        self.denom
    }

    pub fn factor(&self) -> IntCst {
        self.factor
    }

    pub fn is_or_zero(&self) -> bool {
        self.or_zero
    }

    pub fn var(&self) -> IVar {
        self.var
    }
}

impl From<IVar> for LinearTerm {
    fn from(var: IVar) -> Self {
        LinearTerm::new(1, var, false)
    }
}

impl std::ops::Neg for LinearTerm {
    type Output = LinearTerm;

    fn neg(self) -> Self::Output {
        LinearTerm {
            factor: -self.factor,
            var: self.var,
            or_zero: self.or_zero,
            denom: self.denom,
        }
    }
}

/// A linear sum of the form `a1/b1 * X1 + a2/b2 * X2 + ... + Y` where `ai`, `bi` and `Y` are constants and `Xi` is a variable.
#[derive(Clone, Debug)]
pub struct LinearSum {
    terms: Vec<LinearTerm>,
    constant: IntCst,
    denom: IntCst,
}

impl LinearSum {
    pub fn zero() -> LinearSum {
        LinearSum {
            terms: Vec::new(),
            constant: 0,
            denom: 1,
        }
    }

    pub fn constant(n: IntCst) -> LinearSum {
        Self::zero() + n
    }

    pub fn of<T: Into<LinearSum>>(elements: Vec<T>) -> LinearSum {
        let mut terms: Vec<LinearTerm> = Vec::with_capacity(elements.len());
        let mut constant = 0;
        for e in elements {
            let sum: LinearSum = e.into();
            terms.extend(sum.terms);
            constant += sum.constant;
        }
        let mut sum = LinearSum {
            terms,
            constant,
            denom: 1,
        };
        sum.update_factors();
        sum
    }

    /// Updates the factors and denominators of the terms so that the denominators are equal.
    fn update_factors(&mut self) {
        let mut denom = 1;
        // Search the least denominator.
        for term in self.terms.clone() {
            denom = lcm(denom, term.denom);
        }
        // Apply the denominator to each term.
        for term in self.terms.as_mut_slice() {
            term.factor *= denom / term.denom;
            term.denom = denom;
        }
        // Store the denominator
        self.denom = denom
    }

    pub fn leq<T: Into<LinearSum>>(self, upper_bound: T) -> LinearLeq {
        LinearLeq::new(self - upper_bound, 0)
    }
    pub fn geq<T: Into<LinearSum>>(self, lower_bound: T) -> LinearLeq {
        (-self).leq(-lower_bound.into())
    }

    pub fn get_constant(&self) -> IntCst {
        self.constant
    }

    pub fn denom(&self) -> IntCst {
        self.denom
    }

    pub fn terms(&self) -> &[LinearTerm] {
        self.terms.as_ref()
    }
}

impl From<LinearTerm> for LinearSum {
    fn from(term: LinearTerm) -> Self {
        LinearSum {
            terms: vec![term],
            constant: 0,
            denom: 1,
        }
    }
}
impl From<IntCst> for LinearSum {
    fn from(constant: IntCst) -> Self {
        LinearSum {
            terms: Vec::new(),
            constant,
            denom: 1,
        }
    }
}
impl From<FAtom> for LinearSum {
    fn from(value: FAtom) -> Self {
        LinearSum {
            terms: vec![LinearTerm {
                factor: 1,
                var: value.num.var,
                or_zero: false,
                denom: value.denom,
            }],
            constant: value.num.shift,
            denom: value.denom,
        }
    }
}

impl From<IAtom> for LinearSum {
    fn from(value: IAtom) -> Self {
        LinearSum {
            terms: vec![LinearTerm {
                factor: 1,
                var: value.var,
                or_zero: false,
                denom: 1,
            }],
            constant: value.shift,
            denom: 1,
        }
    }
}

impl<T: Into<LinearSum>> std::ops::Add<T> for LinearSum {
    type Output = LinearSum;

    fn add(mut self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        self.terms.extend_from_slice(&rhs.terms);
        self.constant += rhs.constant;
        self.update_factors();
        self
    }
}

impl<T: Into<LinearSum>> std::ops::Sub<T> for LinearSum {
    type Output = LinearSum;

    fn sub(mut self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        self.terms.extend(rhs.terms.iter().map(|t| -*t));
        self.constant -= rhs.constant;
        self.update_factors();
        self
    }
}

impl<T: Into<LinearSum>> std::ops::AddAssign<T> for LinearSum {
    fn add_assign(&mut self, rhs: T) {
        let sum: LinearSum = rhs.into();
        self.terms.extend(sum.terms);
        self.constant += sum.constant;
        self.update_factors();
    }
}
impl<T: Into<LinearSum>> std::ops::SubAssign<T> for LinearSum {
    fn sub_assign(&mut self, rhs: T) {
        let sum: LinearSum = -rhs.into();
        *self += sum;
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

use crate::transitive_conversion;

use super::FAtom;
transitive_conversion!(LinearSum, LinearTerm, IVar);

pub struct LinearLeq {
    sum: LinearSum,
    ub: IntCst,
}

impl LinearLeq {
    pub fn new(sum: LinearSum, ub: IntCst) -> LinearLeq {
        LinearLeq { sum, ub }
    }
}

impl From<LinearLeq> for ReifExpr {
    fn from(value: LinearLeq) -> Self {
        let mut vars = BTreeMap::new();
        for e in &value.sum.terms {
            let var = VarRef::from(e.var);
            let key = (var, e.or_zero);
            vars.entry(key)
                .and_modify(|factor| *factor += e.factor)
                .or_insert(e.factor);
        }
        // TODO: use optimized representation when possible (literal, max-diff, ...)
        ReifExpr::Linear(NFLinearLeq {
            sum: vars
                .iter()
                .map(|(&(var, or_zero), &factor)| NFLinearSumItem { var, factor, or_zero })
                .collect(),
            upper_bound: value.ub - value.sum.constant * value.sum.denom,
        })
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct NFLinearSumItem {
    pub var: VarRef,
    pub factor: IntCst,
    /// If true, the this term should be interpreted as zero if the variable is absent.
    pub or_zero: bool,
}

impl std::ops::Neg for NFLinearSumItem {
    type Output = NFLinearSumItem;

    fn neg(self) -> Self::Output {
        NFLinearSumItem {
            var: self.var,
            factor: -self.factor,
            or_zero: self.or_zero,
        }
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct NFLinearLeq {
    pub sum: Vec<NFLinearSumItem>,
    pub upper_bound: IntCst,
}

impl NFLinearLeq {
    pub(crate) fn validity_scope(&self, presence: impl Fn(VarRef) -> Lit) -> ValidityScope {
        // the expression is valid if all variables are present, except for those that do not evaluate to zero when absent
        let required_presence: Vec<Lit> = self
            .sum
            .iter()
            .filter(|item| !item.or_zero)
            .map(|item| presence(item.var))
            .collect();
        ValidityScope::new(required_presence, [])
    }

    /// Returns a new `NFLinearLeq` without the items of the sum with a null `factor`.
    pub(crate) fn simplify(&self) -> NFLinearLeq {
        NFLinearLeq {
            sum: self
                .sum
                .clone()
                .into_iter()
                .filter(|x| x.factor != 0 && x.var != VarRef::ZERO)
                .collect::<Vec<_>>(),
            upper_bound: self.upper_bound,
        }
    }
}

impl std::ops::Not for NFLinearLeq {
    type Output = Self;

    fn not(mut self) -> Self::Output {
        // not(a + b <= ub)  <=>  -a -b <= -ub -1
        self.sum.iter_mut().for_each(|i| *i = -*i);
        NFLinearLeq {
            sum: self.sum,
            upper_bound: -self.upper_bound - 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::lang::FAtom;

    use super::*;

    fn check_term(t: LinearTerm, f: IntCst, d: IntCst) {
        assert_eq!(t.factor, f);
        assert_eq!(t.denom, d);
    }

    fn check_sum(s: LinearSum, t: Vec<(IntCst, IntCst)>, c: IntCst) {
        assert_eq!(s.constant, c);
        for i in 0..s.terms.len() {
            check_term(s.terms[i], t[i].0, t[i].1);
        }
    }

    #[test]
    fn test_term_from_ivar() {
        let var = IVar::ZERO;
        let term = LinearTerm::from(var);
        check_term(term, 1, 1);
    }

    #[test]
    fn test_term_neg() {
        let term = -LinearTerm::from(IVar::ZERO);
        check_term(term, -1, 1);
    }

    #[test]
    fn test_sum_from_fatom() {
        let atom = FAtom::new(5.into(), 10);
        let sum = LinearSum::from(atom);
        check_sum(sum, vec![(1, 10)], 5);
    }

    #[test]
    fn test_sum_of_elements_same_denom() {
        let elements = vec![FAtom::new(5.into(), 10), FAtom::new(10.into(), 10)];
        let sum = LinearSum::of(elements);
        check_sum(sum, vec![(1, 10), (1, 10)], 15);
    }

    #[test]
    fn test_sum_of_elements_different_denom() {
        let elements = vec![
            LinearSum::from(FAtom::new(5.into(), 28)),
            LinearSum::from(FAtom::new(10.into(), 77)),
            -LinearSum::from(FAtom::new(3.into(), 77)),
        ];
        let sum = LinearSum::of(elements);
        check_sum(sum, vec![(11, 308), (4, 308), (-4, 308)], 12);
    }

    #[test]
    fn test_sum_add() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        check_sum(s1.clone(), vec![(1, 28)], 5);
        check_sum(s2.clone(), vec![(1, 77)], 10);
        check_sum(s1 + s2, vec![(11, 308), (4, 308)], 15);
    }

    #[test]
    fn test_sum_sub() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        check_sum(s1.clone(), vec![(1, 28)], 5);
        check_sum(s2.clone(), vec![(1, 77)], 10);
        check_sum(s1 - s2, vec![(11, 308), (-4, 308)], -5);
    }

    #[test]
    fn test_sum_add_assign() {
        let mut s = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        check_sum(s.clone(), vec![(1, 28)], 5);
        s += FAtom::new(10.into(), 77);
        check_sum(s, vec![(11, 308), (4, 308)], 15);
    }

    #[test]
    fn test_sum_sub_assign() {
        let mut s = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        check_sum(s.clone(), vec![(1, 28)], 5);
        s -= FAtom::new(10.into(), 77);
        check_sum(s, vec![(11, 308), (-4, 308)], -5);
    }

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

    #[test]
    fn test_cleaner_nflinear_leq() {
        let var = VarRef::from_u32(5);
        let nll = NFLinearLeq {
            sum: vec![
                NFLinearSumItem {
                    var: VarRef::ZERO,
                    factor: 1,
                    or_zero: false,
                },
                NFLinearSumItem {
                    var: var,
                    factor: 0,
                    or_zero: false,
                },
                NFLinearSumItem {
                    var: var,
                    factor: 1,
                    or_zero: false,
                },
            ],
            upper_bound: 5,
        };
        let exp = NFLinearLeq {
            sum: vec![NFLinearSumItem {
                var: var,
                factor: 1,
                or_zero: false,
            }],
            upper_bound: 5,
        };
        assert_eq!(nll.simplify(), exp);
    }
}
