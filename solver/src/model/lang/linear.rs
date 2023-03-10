use crate::core::{IntCst, Lit, VarRef};
use crate::model::lang::{IVar, ValidityScope};
use crate::reif::ReifExpr;
use std::collections::BTreeMap;

/// A linear term of the form `(a * X) + b` where `a` and `b` are constants and `X` is a variable.
#[derive(Copy, Clone, Debug)]
pub struct LinearTerm {
    factor: IntCst,
    var: IVar,
    /// If true, then this term should be interpreted as zero if the variable is absent.
    or_zero: bool,
}

impl LinearTerm {
    pub const fn new(factor: IntCst, var: IVar, or_zero: bool) -> LinearTerm {
        LinearTerm { factor, var, or_zero }
    }

    pub fn or_zero(self) -> Self {
        LinearTerm {
            factor: self.factor,
            var: self.var,
            or_zero: true,
        }
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
        LinearTerm::new(-self.factor, self.var, self.or_zero)
    }
}

#[derive(Clone, Debug)]
pub struct LinearSum {
    terms: Vec<LinearTerm>,
    constant: IntCst,
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
        LinearSum {
            terms: vec,
            constant: 0,
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
        self
    }
}

impl<T: Into<LinearSum>> std::ops::Sub<T> for LinearSum {
    type Output = LinearSum;

    fn sub(mut self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        self.terms.extend(rhs.terms.iter().map(|t| -*t));
        self.constant -= rhs.constant;
        self
    }
}

impl<T: Into<LinearTerm>> std::ops::AddAssign<T> for LinearSum {
    fn add_assign(&mut self, rhs: T) {
        self.terms.push(rhs.into())
    }
}
impl<T: Into<LinearTerm>> std::ops::SubAssign<T> for LinearSum {
    fn sub_assign(&mut self, rhs: T) {
        self.terms.push(-rhs.into())
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
