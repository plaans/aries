use crate::lang::expr::Normalize;
use crate::lang::normal_form::NormalExpr;
use crate::lang::reification::ExprInterface;
use crate::lang::{IVar, ValidityScope};
use aries_core::{IntCst, Lit, VarRef};
use std::collections::BTreeMap;

/// A linear term of the form `(a * X) + b` where `a` and `b` are constants and `X` is a variable.
#[derive(Copy, Clone, Debug)]
pub struct LinearTerm {
    factor: IntCst,
    var: IVar,
    cst: IntCst,
}

impl LinearTerm {
    pub const fn new(factor: IntCst, var: IVar, cst: IntCst) -> LinearTerm {
        LinearTerm { factor, var, cst }
    }
}

impl From<IntCst> for LinearTerm {
    fn from(b: IntCst) -> Self {
        LinearTerm::new(0, IVar::ZERO, b)
    }
}
impl From<IVar> for LinearTerm {
    fn from(var: IVar) -> Self {
        LinearTerm::new(1, var, 0)
    }
}

impl std::ops::Neg for LinearTerm {
    type Output = LinearTerm;

    fn neg(self) -> Self::Output {
        LinearTerm::new(-self.factor, self.var, -self.cst)
    }
}

#[derive(Clone, Debug)]
pub struct LinearSum {
    elements: Vec<LinearTerm>,
}

impl LinearSum {
    pub fn zero() -> LinearSum {
        LinearSum { elements: Vec::new() }
    }
    pub fn of<T: Into<LinearTerm>>(elements: Vec<T>) -> LinearSum {
        let mut vec = Vec::with_capacity(elements.len());
        for e in elements {
            vec.push(e.into());
        }
        LinearSum { elements: vec }
    }

    pub fn leq<T: Into<LinearTerm>>(self, upper_bound: T) -> LinearLeq {
        LinearLeq::new(self - upper_bound, 0)
    }
    pub fn geq<T: Into<LinearTerm>>(self, lower_bound: T) -> LinearLeq {
        (-self).leq(-lower_bound.into())
    }
}

impl<T: Into<LinearTerm>> std::ops::Add<T> for LinearSum {
    type Output = LinearSum;

    fn add(mut self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        self.elements.push(rhs);
        self
    }
}

impl<T: Into<LinearTerm>> std::ops::Sub<T> for LinearSum {
    type Output = LinearSum;

    fn sub(self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        self + (-rhs)
    }
}

impl<T: Into<LinearTerm>> std::ops::AddAssign<T> for LinearSum {
    fn add_assign(&mut self, rhs: T) {
        self.elements.push(rhs.into())
    }
}
impl<T: Into<LinearTerm>> std::ops::SubAssign<T> for LinearSum {
    fn sub_assign(&mut self, rhs: T) {
        self.elements.push(rhs.into())
    }
}

impl std::ops::Neg for LinearSum {
    type Output = LinearSum;

    fn neg(mut self) -> Self::Output {
        for e in &mut self.elements {
            *e = -(*e)
        }
        self
    }
}

pub struct LinearLeq {
    sum: LinearSum,
    ub: IntCst,
}

impl LinearLeq {
    pub fn new(sum: LinearSum, ub: IntCst) -> LinearLeq {
        LinearLeq { sum, ub }
    }
}

impl Normalize<NFLinearLeq> for LinearLeq {
    fn normalize(&self) -> NormalExpr<NFLinearLeq> {
        let sum_constant: IntCst = self.sum.elements.iter().map(|e| e.cst).sum();
        let mut vars = BTreeMap::new();
        for e in &self.sum.elements {
            vars.entry(VarRef::from(e.var))
                .and_modify(|factor| *factor += e.factor)
                .or_insert(e.factor);
        }
        NormalExpr::Pos(NFLinearLeq {
            sum: vars
                .iter()
                .map(|(&var, &factor)| NFLinearSumItem { var, factor })
                .collect(),
            upper_bound: self.ub - sum_constant,
        })
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct NFLinearSumItem {
    pub var: VarRef,
    pub factor: IntCst,
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub struct NFLinearLeq {
    pub sum: Vec<NFLinearSumItem>,
    pub upper_bound: IntCst,
}

impl ExprInterface for NFLinearLeq {
    fn validity_scope(&self, _presence: &dyn Fn(VarRef) -> Lit) -> ValidityScope {
        // always valid due to assumptions on the presence of variables
        ValidityScope::EMPTY
    }
}
