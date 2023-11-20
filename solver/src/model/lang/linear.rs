use num_integer::lcm;

use crate::core::{IntCst, Lit, VarRef};
use crate::model::lang::{IAtom, IVar, ValidityScope};
use crate::reif::ReifExpr;
use std::collections::BTreeMap;

/* ========================================================================== */
/*                                 LinearTerm                                 */
/* ========================================================================== */

/// A linear term of the form `a/b * X * P` where:
///  - `a` and `b` are integer constants
///  - `X` is an integer variable.
///  - `P` is a non-optional literal interpreted as 0 if False and 1 if true.
///
/// If `P` is true, it **required** that the expression is defined, meaning that both `X` and `P` are present.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LinearTerm {
    factor: IntCst,
    var: IVar,
    /// If true, then the variable must be present. Otherwise, the term is evaluated to 0.
    lit: Lit,
    denom: IntCst,
}

impl std::fmt::Display for LinearTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.factor != 1 {
            if self.factor < 0 {
                write!(f, "({})", self.factor)?;
            } else {
                write!(f, "{}", self.factor)?;
            }
            write!(f, "*")?;
        }
        if self.var != IVar::ONE {
            write!(f, "{:?}", self.var)?;
        }
        write!(f, "[{:?}]", self.lit)
    }
}

impl LinearTerm {
    pub const fn new(factor: IntCst, var: IVar, lit: Lit, denom: IntCst) -> LinearTerm {
        LinearTerm {
            factor,
            var,
            lit,
            denom,
        }
    }

    pub const fn int(factor: IntCst, var: IVar, lit: Lit) -> LinearTerm {
        LinearTerm {
            factor,
            var,
            lit,
            denom: 1,
        }
    }

    pub const fn rational(factor: IntCst, var: IVar, denom: IntCst, lit: Lit) -> LinearTerm {
        LinearTerm {
            factor,
            var,
            lit,
            denom,
        }
    }

    pub const fn constant_int(value: IntCst, lit: Lit) -> LinearTerm {
        LinearTerm {
            factor: value,
            var: IVar::ONE,
            lit,
            denom: 1,
        }
    }

    pub const fn constant_rational(num: IntCst, denom: IntCst, lit: Lit) -> LinearTerm {
        LinearTerm {
            factor: num,
            var: IVar::ONE,
            lit,
            denom,
        }
    }

    pub fn denom(&self) -> IntCst {
        self.denom
    }

    pub fn factor(&self) -> IntCst {
        self.factor
    }

    pub fn lit(&self) -> Lit {
        self.lit
    }

    pub fn var(&self) -> IVar {
        self.var
    }
}

impl From<IVar> for LinearTerm {
    fn from(var: IVar) -> Self {
        LinearTerm::int(1, var, Lit::TRUE)
    }
}

impl From<IntCst> for LinearTerm {
    fn from(value: IntCst) -> Self {
        LinearTerm::constant_int(value, Lit::TRUE)
    }
}

impl std::ops::Neg for LinearTerm {
    type Output = LinearTerm;

    fn neg(self) -> Self::Output {
        LinearTerm {
            factor: -self.factor,
            var: self.var,
            lit: self.lit,
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
            if i != 0 {
                write!(f, " + ")?;
            }
            write!(f, "{e}")?;
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

    pub fn with_lit<T: Into<LinearSum>>(value: T, lit: Lit) -> LinearSum {
        let sum: LinearSum = value.into();
        sum.map_with_lit(|_| lit)
    }

    /// Returns a copy of the linear sum where the literals are updated according to the mapping.
    pub fn map_with_lit<F>(&self, mut map: F) -> LinearSum
    where
        F: FnMut(&LinearTerm) -> Lit,
    {
        let mut sum = self.clone();
        sum.terms.iter_mut().for_each(|t| t.lit = map(t));
        sum
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
        let new_denom = lcm(self.denom, added.denom);
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
            // Group the terms by their `variable` and `lit` attribute.
            term_map
                .entry((term.lit, term.var))
                .and_modify(|f| *f += term.factor)
                .or_insert(term.factor);

            // Group the constant terms into the constant.
            if term.var == IVar::ONE && term.lit == Lit::TRUE {
                constant += term.factor;
            }

            // By creation, all terms should have the same denom. Check it.
            debug_assert_eq!(term.denom, self.denom);
        }

        // Filter the null `factor`, the `variable` ZERO, and the constant terms.
        LinearSum {
            constant,
            denom: self.denom,
            terms: term_map
                .into_iter()
                .filter(|((_, v), f)| *f != 0 && *v != IVar::ZERO)
                .filter(|((z, v), _)| !(*v == IVar::ONE && *z == Lit::TRUE)) // Has been grouped into the constant
                .map(|((z, v), f)| LinearTerm {
                    factor: f,
                    var: v,
                    lit: z,
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

impl From<FAtom> for LinearSum {
    fn from(value: FAtom) -> Self {
        let mut sum = LinearSum {
            terms: vec![LinearTerm {
                factor: 1,
                var: value.num.var,
                lit: Lit::TRUE,
                denom: value.denom,
            }],
            constant: 0,
            denom: value.denom,
        };
        sum += LinearTerm::constant_rational(value.num.shift, value.denom, Lit::TRUE);
        sum
    }
}

impl From<IAtom> for LinearSum {
    fn from(value: IAtom) -> Self {
        let mut sum = LinearSum {
            terms: vec![LinearTerm {
                factor: 1,
                var: value.var,
                lit: Lit::TRUE,
                denom: 1,
            }],
            constant: 0,
            denom: 1,
        };
        sum += LinearTerm::constant_int(value.shift, Lit::TRUE);
        sum
    }
}

impl TryFrom<Atom> for LinearSum {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Int(i) => Ok(LinearSum::from(i)),
            Atom::Fixed(f) => Ok(LinearSum::from(f)),
            _ => Err(ConversionError::TypeError),
        }
    }
}

impl<T: Into<LinearSum>> std::ops::Add<T> for LinearSum {
    type Output = LinearSum;

    fn add(self, rhs: T) -> Self::Output {
        let mut new = self;
        new += rhs.into();
        new
    }
}

impl<T: Into<LinearSum>> std::ops::Sub<T> for LinearSum {
    type Output = LinearSum;

    fn sub(self, rhs: T) -> Self::Output {
        self + (-rhs.into())
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

use super::{Atom, ConversionError, FAtom};
transitive_conversion!(LinearSum, LinearTerm, IVar);

/* ========================================================================== */
/*                                  LinearLeq                                 */
/* ========================================================================== */

pub struct LinearLeq {
    sum: LinearSum,
    ub: IntCst,
}

impl std::fmt::Display for LinearLeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <= {}", self.sum, self.ub)
    }
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
            let key = (var, e.lit);
            vars.entry(key)
                .and_modify(|factor| *factor += e.factor)
                .or_insert(e.factor);
        }
        ReifExpr::Linear(NFLinearLeq {
            sum: vars
                .iter()
                .map(|(&(var, lit), &factor)| NFLinearSumItem { var, factor, lit })
                .collect(),
            upper_bound: value.ub - value.sum.constant,
        })
    }
}

/* ========================================================================== */
/*                               NFLinearSumItem                              */
/* ========================================================================== */

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct NFLinearSumItem {
    pub var: VarRef,
    pub factor: IntCst,
    /// If true, then the variable should be present. Otherwise, the term is ignored.
    pub lit: Lit,
}

impl std::fmt::Display for NFLinearSumItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.factor != 1 {
            if self.factor < 0 {
                write!(f, "({})", self.factor)?;
            } else {
                write!(f, "{}", self.factor)?;
            }
            write!(f, "*")?;
        }
        if self.var != VarRef::ONE {
            write!(f, "{:?}", self.var)?;
        }
        write!(f, "[{:?}]", self.lit)
    }
}

impl std::ops::Neg for NFLinearSumItem {
    type Output = NFLinearSumItem;

    fn neg(self) -> Self::Output {
        NFLinearSumItem {
            var: self.var,
            factor: -self.factor,
            lit: self.lit,
        }
    }
}

/* ========================================================================== */
/*                                 NFLinearLeq                                */
/* ========================================================================== */

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct NFLinearLeq {
    pub sum: Vec<NFLinearSumItem>,
    pub upper_bound: IntCst,
}

impl std::fmt::Display for NFLinearLeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.sum.iter().enumerate() {
            if i != 0 {
                write!(f, " + ")?;
            }
            write!(f, "{e}")?;
        }
        write!(f, " <= {}", self.upper_bound)
    }
}

impl NFLinearLeq {
    pub(crate) fn validity_scope(&self, presence: impl Fn(VarRef) -> Lit) -> ValidityScope {
        // the expression is valid if all variables are present, except for those that do not evaluate to zero when absent
        let required_presence: Vec<Lit> = self
            .sum
            .iter()
            .filter(|item| item.lit == Lit::TRUE)
            .map(|item| presence(item.var))
            .collect();
        ValidityScope::new(required_presence, [])
    }

    /// Returns a new `NFLinearLeq` without the terms with a null `factor` or the `variable` ZERO.
    /// The terms are grouped by (`variable`, `lit`) and the constant terms and grouped into the `upper_bound`.
    pub(crate) fn simplify(&self) -> NFLinearLeq {
        let mut sum_map = BTreeMap::new();
        let mut upper_bound = self.upper_bound;
        for term in &self.sum {
            // Group the terms by their `variable` and `lit` attribute.
            sum_map
                .entry((term.lit, term.var))
                .and_modify(|f| *f += term.factor)
                .or_insert(term.factor);

            // Group the constant terms into the `upper_bound`.
            if term.var == VarRef::ONE && term.lit == Lit::TRUE {
                upper_bound -= term.factor;
            }
        }
        // Filter the null `factor`, the `variable` ZERO, and the constant terms (null `variable` with true `lit`).
        NFLinearLeq {
            sum: sum_map
                .into_iter()
                .filter(|((_, v), f)| *f != 0 && *v != VarRef::ZERO)
                .filter(|((z, v), _)| !(*v == VarRef::ONE && *z == Lit::TRUE)) // Has been grouped into the upper bound
                .map(|((z, v), f)| NFLinearSumItem {
                    var: v,
                    factor: f,
                    lit: z,
                })
                .collect(),
            upper_bound,
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

/* ========================================================================== */
/*                                 Unit Tests                                 */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use crate::model::lang::FAtom;

    use super::*;

    /* ========================== LinearTerm Tests ========================== */

    #[test]
    fn test_term_new() {
        let var1 = IVar::new(VarRef::from_u32(5));
        let var2 = IVar::new(VarRef::from_u32(15));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for l in [Lit::TRUE, Lit::FALSE, var1.leq(2)] {
                        for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                            let term = LinearTerm::new(ff * f, v, l, d);
                            assert_eq!(term.factor, ff * f);
                            assert_eq!(term.var, v);
                            assert_eq!(term.lit, l);
                            assert_eq!(term.denom, d);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_int() {
        let var1 = IVar::new(VarRef::from_u32(5));
        let var2 = IVar::new(VarRef::from_u32(15));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [var1, var2] {
                    for l in [Lit::TRUE, Lit::FALSE, var1.leq(2)] {
                        let term = LinearTerm::int(ff * f, v, l);
                        assert_eq!(term.factor, ff * f);
                        assert_eq!(term.var, v);
                        assert_eq!(term.lit, l);
                        assert_eq!(term.denom, 1);
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_rational() {
        let var1 = IVar::new(VarRef::from_u32(5));
        let var2 = IVar::new(VarRef::from_u32(15));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [var1, var2] {
                    for l in [Lit::TRUE, Lit::FALSE, var1.leq(2)] {
                        for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                            let term = LinearTerm::rational(ff * f, v, d, l);
                            assert_eq!(term.factor, ff * f);
                            assert_eq!(term.var, v);
                            assert_eq!(term.lit, l);
                            assert_eq!(term.denom, d);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_constant_int() {
        let var = IVar::new(VarRef::from_u32(5));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for l in [Lit::TRUE, Lit::FALSE, var.leq(2)] {
                    let term = LinearTerm::constant_int(ff * f, l);
                    assert_eq!(term.factor, ff * f);
                    assert_eq!(term.var, IVar::ONE);
                    assert_eq!(term.lit, l);
                    assert_eq!(term.denom, 1);
                }
            }
        }
    }

    #[test]
    fn test_term_constant_rational() {
        let var = IVar::new(VarRef::from_u32(5));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for l in [Lit::TRUE, Lit::FALSE, var.leq(2)] {
                    for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                        let term = LinearTerm::constant_rational(ff * f, d, l);
                        assert_eq!(term.factor, ff * f);
                        assert_eq!(term.var, IVar::ONE);
                        assert_eq!(term.lit, l);
                        assert_eq!(term.denom, d);
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_from_ivar() {
        let var0 = IVar::ZERO;
        let var1 = IVar::ONE;
        let var2 = IVar::new(VarRef::from_u32(5));
        let var3 = IVar::new(VarRef::from_u32(15));
        for v in [var0, var1, var2, var3] {
            let term = LinearTerm::from(v);
            let expected = LinearTerm::int(1, v, Lit::TRUE);
            assert_eq!(term, expected);
        }
    }

    #[test]
    fn test_term_from_int_cst() {
        for i in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            let term = LinearTerm::from(i);
            let expected = LinearTerm::constant_int(i, Lit::TRUE);
            assert_eq!(term, expected);
        }
    }

    #[test]
    fn test_term_eq() {
        let mut terms: Vec<LinearTerm> = vec![];
        let var1 = IVar::new(VarRef::from_u32(5));
        let var2 = IVar::new(VarRef::from_u32(15));
        for f in [1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for l in [Lit::TRUE, Lit::FALSE, var1.leq(2)] {
                        for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                            let term = LinearTerm::new(ff * f, v, l, d);
                            terms.push(term);
                        }
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
        let var1 = IVar::new(VarRef::from_u32(5));
        let var2 = IVar::new(VarRef::from_u32(15));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for l in [Lit::TRUE, Lit::FALSE, var1.leq(2)] {
                        for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                            let term = -LinearTerm::new(ff * f, v, l, d);
                            let expected = LinearTerm::new(-ff * f, v, l, d);
                            assert_eq!(term, expected);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_term_getters() {
        let var1 = IVar::new(VarRef::from_u32(5));
        let var2 = IVar::new(VarRef::from_u32(15));
        for f in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for ff in [-1, 1] {
                for v in [IVar::ONE, var1, var2] {
                    for l in [Lit::TRUE, Lit::FALSE, var1.leq(2)] {
                        for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                            let term = LinearTerm::new(ff * f, v, l, d);
                            assert_eq!(term.factor, term.factor());
                            assert_eq!(term.var, term.var());
                            assert_eq!(term.lit, term.lit());
                            assert_eq!(term.denom, term.denom());
                        }
                    }
                }
            }
        }
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
    fn test_sum_with_lit() {
        let var = IVar::new(VarRef::from_u32(5));
        let terms = vec![
            LinearTerm::rational(1, var, 10, Lit::TRUE),
            LinearTerm::constant_rational(5, 10, Lit::TRUE),
        ];
        let sum = LinearSum::of(terms);
        for l in [var.geq(2), var.leq(6), Lit::FALSE, Lit::TRUE] {
            let new_sum = LinearSum::with_lit(sum.clone(), l);
            assert_eq!(new_sum.constant, sum.constant);
            assert_eq!(new_sum.denom, sum.denom);
            for (t, nt) in sum.terms.iter().zip(new_sum.terms) {
                assert_eq!(nt.factor, t.factor);
                assert_eq!(nt.var, t.var);
                assert_eq!(nt.denom, t.denom);
                assert_eq!(nt.lit, l);
            }
        }
    }

    #[test]
    fn test_sum_map_with_lit() {
        let var = IVar::new(VarRef::from_u32(5));

        let t1 = LinearTerm::rational(1, var, 10, Lit::TRUE);
        let t2 = LinearTerm::constant_rational(5, 10, Lit::TRUE);
        let sum = LinearSum::of([t1, t2].to_vec());
        for l1 in [var.geq(2), var.leq(6), Lit::FALSE, Lit::TRUE] {
            for l2 in [var.geq(2), var.leq(6), Lit::FALSE, Lit::TRUE] {
                let new_sum = sum.map_with_lit(|t| {
                    if *t == t1 {
                        return l1;
                    }
                    l2
                });
                assert_eq!(new_sum.constant, sum.constant);
                assert_eq!(new_sum.denom, sum.denom);
                for (t, nt) in sum.terms.iter().zip(new_sum.terms) {
                    assert_eq!(nt.factor, t.factor);
                    assert_eq!(nt.var, t.var);
                    assert_eq!(nt.denom, t.denom);
                    if *t == t1 {
                        assert_eq!(nt.lit, l1);
                    } else {
                        assert_eq!(nt.lit, l2);
                    }
                }
            }
        }
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
        let var = IVar::new(VarRef::from_u32(5));
        let terms = vec![
            LinearTerm::rational(1, var, 10, Lit::TRUE),
            LinearTerm::constant_rational(5, 10, Lit::TRUE),
        ];
        let sum = LinearSum::of(terms.clone());
        assert_eq!(sum.constant, 0);
        assert_eq!(sum.denom, 10);
        assert_eq!(sum.terms, terms);
    }

    #[test]
    fn test_sum_of_elements_different_denom() {
        let terms = vec![
            LinearTerm::constant_rational(5, 28, Lit::TRUE),
            LinearTerm::constant_rational(10, 77, Lit::TRUE),
            LinearTerm::constant_rational(-3, 77, Lit::TRUE),
        ];

        let expected_terms = vec![
            LinearTerm::constant_rational(55, 308, Lit::TRUE),
            LinearTerm::constant_rational(40, 308, Lit::TRUE),
            LinearTerm::constant_rational(-12, 308, Lit::TRUE),
        ];
        let sum = LinearSum::of(terms);
        assert_eq!(sum.constant, 0);
        assert_eq!(sum.denom, 308);
        assert_eq!(sum.terms, expected_terms);
    }

    #[test]
    fn test_sum_set_denom() {
        let terms = [
            LinearTerm::constant_rational(5, 28, Lit::TRUE),
            LinearTerm::constant_rational(10, 77, Lit::TRUE),
        ];
        let expected_terms = vec![
            LinearTerm::constant_rational(55, 308, Lit::TRUE),
            LinearTerm::constant_rational(40, 308, Lit::TRUE),
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
        sum.add_term(LinearTerm::constant_rational(5, 28, Lit::TRUE));
        assert_eq!(sum.constant, 12);
        assert_eq!(sum.denom, 308);
        assert_eq!(sum.terms, vec![LinearTerm::constant_rational(55, 308, Lit::TRUE)]);
    }

    #[test]
    fn test_sum_add_rational() {
        let mut sum = LinearSum::of(vec![LinearTerm::constant_rational(5, 28, Lit::TRUE)]);
        sum.add_rational(3, 77);
        assert_eq!(sum.constant, 12);
        assert_eq!(sum.denom, 308);
        assert_eq!(sum.terms, vec![LinearTerm::constant_rational(55, 308, Lit::TRUE)]);
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
        let var = IVar::new(VarRef::from_u32(5));
        let terms = vec![
            LinearTerm::rational(1, var, 10, Lit::TRUE),
            LinearTerm::constant_rational(5, 10, Lit::TRUE),
        ];
        let sum = LinearSum::of(terms.clone());
        assert_eq!(sum.constant(), sum.constant);
        assert_eq!(sum.denom(), sum.denom);
        assert_eq!(sum.terms(), sum.terms);
    }

    #[test]
    fn test_sum_simplify() {
        // Terms should be grouped by (lit, variable)
        // Terms with null `factor` or `variable` equals to VarRef::ZERO should be filtered
        // Terms with null `variable` and `literal` equals to Lit::TRUE  should be grouped into the constant
        let denom = 100;
        let var0 = IVar::ZERO;
        let var1 = IVar::new(VarRef::from_u32(5));
        let lit0 = Lit::TRUE;
        let lit1 = var1.leq(5);

        let sum = LinearSum {
            constant: 5,
            denom,
            terms: vec![
                // Constant terms with true lit, should be in the constant
                LinearTerm::new(10, IVar::ONE, lit0, denom),
                LinearTerm::new(15, IVar::ONE, lit0, denom),
                // Constant terms without true lit, should be grouped
                LinearTerm::new(20, IVar::ONE, lit1, denom),
                LinearTerm::new(25, IVar::ONE, lit1, denom),
                // Variable terms with zero variable, should be filtered
                LinearTerm::new(30, var0, lit0, denom),
                LinearTerm::new(35, var0, lit0, denom),
                LinearTerm::new(40, var0, lit1, denom),
                LinearTerm::new(45, var0, lit1, denom),
                // Variable terms with null factor, should be filtered
                LinearTerm::new(0, var1, lit0, denom),
                LinearTerm::new(0, var1, lit0, denom),
                LinearTerm::new(0, var1, lit1, denom),
                LinearTerm::new(0, var1, lit1, denom),
                // Other variable terms no specificities, should be grouped by lit
                LinearTerm::new(50, var1, lit0, denom),
                LinearTerm::new(55, var1, lit0, denom),
                LinearTerm::new(60, var1, lit1, denom),
                LinearTerm::new(65, var1, lit1, denom),
            ],
        }
        .simplify();

        assert_eq!(sum.constant, 30);
        assert_eq!(sum.denom, 100);

        // Terms could have been reorganized
        let expected_terms = [
            LinearTerm::new(45, IVar::ONE, lit1, denom),
            // Other variable terms no specificities, should be grouped by lit
            LinearTerm::new(105, var1, lit0, denom),
            LinearTerm::new(125, var1, lit1, denom),
        ];
        assert_eq!(sum.terms.len(), expected_terms.len());
        for term in sum.terms {
            assert!(expected_terms.contains(&term));
        }
    }

    #[test]
    fn test_sum_from_linear_term() {
        let terms = vec![
            LinearTerm::constant_rational(5, 28, Lit::TRUE),
            LinearTerm::constant_rational(10, 77, Lit::TRUE),
            LinearTerm::constant_rational(-3, 77, Lit::TRUE),
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
        let var2 = IVar::new(VarRef::from_u32(5));
        let var3 = IVar::new(VarRef::from_u32(15));
        for s in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for d in [1, 2, 5, 10, 15, 20, 50, 100] {
                for v in [var0, var1, var2, var3] {
                    let fa = FAtom::new(IAtom::new(v, s), d);
                    let sum = LinearSum::from(fa);
                    assert_eq!(sum.constant, 0);
                    assert_eq!(sum.denom, d);
                    assert_eq!(
                        sum.terms,
                        vec![
                            LinearTerm::new(1, v, Lit::TRUE, d),
                            LinearTerm::constant_rational(s, d, Lit::TRUE),
                        ]
                    );
                }
            }
        }
    }

    #[test]
    fn test_sum_from_iatom() {
        let var0 = IVar::ZERO;
        let var1 = IVar::ONE;
        let var2 = IVar::new(VarRef::from_u32(5));
        let var3 = IVar::new(VarRef::from_u32(15));
        for s in [0, 1, 2, 5, 10, 15, 20, 50, 100] {
            for v in [var0, var1, var2, var3] {
                let ia = IAtom::new(v, s);
                let sum = LinearSum::from(ia);
                assert_eq!(sum.constant, 0);
                assert_eq!(sum.denom, 1);
                assert_eq!(
                    sum.terms,
                    vec![
                        LinearTerm::new(1, v, Lit::TRUE, 1),
                        LinearTerm::constant_int(s, Lit::TRUE),
                    ]
                );
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

    /* ============================= NFLinearLeq ============================ */

    #[test]
    fn test_simplify_nflinear_leq() {
        // Terms should be grouped by (lit, variable)
        // Terms with null `factor` or `variable` equals to VarRef::ZERO should be filtered
        // Terms with null `variable` and `literal` equals to Lit::TRUE  should be grouped into the upper bound
        let var0 = VarRef::ZERO;
        let var1 = VarRef::from_u32(5);
        let lit0 = Lit::TRUE;
        let lit1 = var1.leq(5);

        let item = |factor: i32, var: VarRef, lit: Lit| NFLinearSumItem { var, factor, lit };

        let obj = NFLinearLeq {
            sum: vec![
                // Constant terms with true lit, should be in the upper bound
                item(10, VarRef::ONE, lit0),
                item(15, VarRef::ONE, lit0),
                // Constant terms without true lit, should be grouped
                item(20, VarRef::ONE, lit1),
                item(25, VarRef::ONE, lit1),
                // Variable terms with zero variable, should be filtered
                item(30, var0, lit0),
                item(35, var0, lit0),
                item(40, var0, lit1),
                item(45, var0, lit1),
                // Variable terms with null factor, should be filtered
                item(0, var1, lit0),
                item(0, var1, lit0),
                item(0, var1, lit1),
                item(0, var1, lit1),
                // Other variable terms no specificities, should be grouped by lit
                item(50, var1, lit0),
                item(55, var1, lit0),
                item(60, var1, lit1),
                item(65, var1, lit1),
            ],
            upper_bound: 5,
        }
        .simplify();

        assert_eq!(obj.upper_bound, -20);

        // Terms could have been reorganized
        let expected_sum = [
            item(45, VarRef::ONE, lit1),
            // Other variable terms no specificities, should be grouped by lit
            item(105, var1, lit0),
            item(125, var1, lit1),
        ];
        assert_eq!(obj.sum.len(), expected_sum.len());
        for term in obj.sum {
            assert!(expected_sum.contains(&term));
        }
    }
}
