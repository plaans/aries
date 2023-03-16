use crate::core::{IntCst, Lit, VarRef};
use crate::model::lang::{IVar, ValidityScope};
use crate::reif::ReifExpr;
use std::cmp::min;
use std::collections::BTreeMap;
use std::mem::swap;

/// A linear term of the form `(a * X) + b` where `a` and `b` are constants and `X` is a variable.
#[derive(Copy, Clone, Debug)]
pub struct LinearTerm {
    pub factor: IntCst,
    pub var: IVar,
    /// If true, then this term should be interpreted as zero if the variable is absent.
    pub or_zero: bool,
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
}

impl From<IVar> for LinearTerm {
    fn from(var: IVar) -> Self {
        LinearTerm::new(1, var, false)
    }
}

impl From<FAtom> for LinearTerm {
    fn from(value: FAtom) -> Self {
        LinearTerm {
            factor: 1,
            var: value.num.var,
            or_zero: false,
            denom: value.denom,
        }
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

#[derive(Clone, Debug)]
pub struct LinearSum {
    terms: Vec<LinearTerm>,
    constant: IntCst,
}

/// Returns the greatest common divisor.
/// Implementation of the binary Euclidean algorithm.
fn gcd(a: IntCst, b: IntCst) -> IntCst {
    // Base cases: gcd(n, 0) = gcd(0, n) = n
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }

    // gcd(2^i * u, 2^j * v) = 2^k * gcd(u, v) with u, v odd
    // 2^k is the greatest power of two that divides both u and v
    let mut u = a;
    let mut v = b;
    let i = u.trailing_zeros();
    u >>= i;
    let j = v.trailing_zeros();
    v >>= j;
    let k = min(i, j);

    loop {
        debug_assert!(u % 2 == 1);
        debug_assert!(v % 2 == 1);

        // Swap if necessary so u <= v
        if u > v {
            swap(&mut u, &mut v);
        }

        // gcd(u, v) = gcd(|v-u|, min(u,v))
        v -= u;

        // gcd(u, 0) = u
        // The shift is necessary to add back the 2^k factor that was removed before the loop
        if v == 0 {
            return u << k;
        }

        // gcd(u, 2^j * v) = gcd(u, v) with u odd (which is the case here)
        v >>= v.trailing_zeros();
    }
}

/// Returns the least common divisor.
fn lcm(a: IntCst, b: IntCst) -> IntCst {
    b * (a / gcd(a, b))
}

impl LinearSum {
    pub fn zero() -> LinearSum {
        LinearSum {
            terms: Vec::new(),
            constant: 0,
        }
    }
    pub fn constant(n: IntCst) -> LinearSum {
        Self::zero() + n
    }
    pub fn of<T: Into<LinearTerm>>(elements: Vec<T>) -> LinearSum {
        let mut vec = Vec::with_capacity(elements.len());
        for e in elements {
            vec.push(e.into());
        }
        let mut sum = LinearSum {
            terms: vec,
            constant: 0,
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
    }

    pub fn leq<T: Into<LinearSum>>(self, upper_bound: T) -> LinearLeq {
        LinearLeq::new(self - upper_bound, 0)
    }
    pub fn geq<T: Into<LinearSum>>(self, lower_bound: T) -> LinearLeq {
        (-self).leq(-lower_bound.into())
    }
}

impl From<LinearTerm> for LinearSum {
    fn from(term: LinearTerm) -> Self {
        LinearSum {
            terms: vec![term],
            constant: 0,
        }
    }
}
impl From<IntCst> for LinearSum {
    fn from(constant: IntCst) -> Self {
        LinearSum {
            terms: Vec::new(),
            constant,
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

impl<T: Into<LinearTerm>> std::ops::AddAssign<T> for LinearSum {
    fn add_assign(&mut self, rhs: T) {
        self.terms.push(rhs.into());
        self.update_factors();
    }
}
impl<T: Into<LinearTerm>> std::ops::SubAssign<T> for LinearSum {
    fn sub_assign(&mut self, rhs: T) {
        self.terms.push(-rhs.into());
        self.update_factors();
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
transitive_conversion!(LinearSum, LinearTerm, FAtom);

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
            upper_bound: value.ub - value.sum.constant,
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
    fn test_term_from_fatom() {
        let atom = FAtom::new(5.into(), 10);
        let term = LinearTerm::from(atom);
        // FIXME (Roland) Don't take the shift into account.
        check_term(term, 1, 10);
    }

    #[test]
    fn test_term_neg() {
        let atom = FAtom::new(5.into(), 10);
        let term = -LinearTerm::from(atom);
        check_term(term, -1, 10);
    }

    #[test]
    fn test_sum_from_fatom() {
        let atom = FAtom::new(5.into(), 10);
        let sum = LinearSum::from(atom);
        check_sum(sum, vec![(1, 10)], 0); // BUG (Roland) The constant should be 5.
    }

    #[test]
    fn test_sum_of_elements_same_denom() {
        let elements = vec![FAtom::new(5.into(), 10), FAtom::new(10.into(), 10)];
        let sum = LinearSum::of(elements);
        check_sum(sum, vec![(1, 10), (1, 10)], 0);
    }

    #[test]
    fn test_sum_of_elements_different_denom() {
        let elements = vec![
            LinearTerm::from(FAtom::new(5.into(), 28)),
            LinearTerm::from(FAtom::new(10.into(), 77)),
            -LinearTerm::from(FAtom::new(3.into(), 77)),
        ];
        let sum = LinearSum::of(elements);
        check_sum(sum, vec![(11, 308), (4, 308), (-4, 308)], 0);
    }

    #[test]
    fn test_sum_add() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        check_sum(s1.clone(), vec![(1, 28)], 0);
        check_sum(s2.clone(), vec![(1, 77)], 0);
        check_sum(s1 + s2, vec![(11, 308), (4, 308)], 0);
    }

    #[test]
    fn test_sum_sub() {
        let s1 = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        let s2 = LinearSum::of(vec![FAtom::new(10.into(), 77)]);
        check_sum(s1.clone(), vec![(1, 28)], 0);
        check_sum(s2.clone(), vec![(1, 77)], 0);
        check_sum(s1 - s2, vec![(11, 308), (-4, 308)], 0);
    }

    #[test]
    fn test_sum_add_assign() {
        let mut s = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        check_sum(s.clone(), vec![(1, 28)], 0);
        s += FAtom::new(10.into(), 77);
        check_sum(s, vec![(11, 308), (4, 308)], 0);
    }

    #[test]
    fn test_sum_sub_assign() {
        let mut s = LinearSum::of(vec![FAtom::new(5.into(), 28)]);
        check_sum(s.clone(), vec![(1, 28)], 0);
        s -= FAtom::new(10.into(), 77);
        check_sum(s, vec![(11, 308), (-4, 308)], 0);
    }

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(3723, 6711), 3);
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(3, 7), 1);
        assert_eq!(gcd(12, 6), 6);
        assert_eq!(gcd(10, 15), 5);
        assert_eq!(gcd(6209, 4435), 887);
        assert_eq!(gcd(1183, 455), 91)
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
}
