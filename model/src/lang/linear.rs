use crate::lang::expr::Normalize;
use crate::lang::normal_form::NormalExpr;
use crate::lang::reification::ExprInterface;
use crate::lang::{IVar, ValidityScope};
use aries_core::{IntCst, Lit, VarRef};

#[derive(Copy, Clone, Debug)]
pub struct IAtomScaled {
    factor: IntCst,
    item: IVar,
}

impl IAtomScaled {
    pub fn new(factor: IntCst, item: IVar) -> IAtomScaled {
        IAtomScaled { factor, item }
    }
}

#[derive(Clone, Debug)]
pub struct LinearSum {
    elements: Vec<IAtomScaled>,
}

impl LinearSum {
    pub fn zero() -> LinearSum {
        LinearSum { elements: Vec::new() }
    }

    pub fn leq(self, upper_bound: IntCst) -> LinearLeq {
        LinearLeq::new(self, upper_bound)
    }
    pub fn geq(mut self, lower_bound: IntCst) -> LinearLeq {
        for e in &mut self.elements {
            e.factor = -e.factor;
        }
        LinearLeq::new(self, -lower_bound)
    }
}

impl std::ops::Add<IAtomScaled> for LinearSum {
    type Output = LinearSum;

    fn add(mut self, rhs: IAtomScaled) -> Self::Output {
        self.elements.push(rhs);
        self
    }
}

impl std::ops::Sub<IAtomScaled> for LinearSum {
    type Output = LinearSum;

    fn sub(mut self, mut rhs: IAtomScaled) -> Self::Output {
        rhs.factor = -rhs.factor;
        self.elements.push(rhs);
        self
    }
}

impl std::ops::Add<IVar> for LinearSum {
    type Output = LinearSum;

    fn add(mut self, rhs: IVar) -> Self::Output {
        self.elements.push(IAtomScaled::new(1, rhs));
        self
    }
}

impl std::ops::Sub<IVar> for LinearSum {
    type Output = LinearSum;

    fn sub(mut self, rhs: IVar) -> Self::Output {
        self.elements.push(IAtomScaled::new(-1, rhs));
        self
    }
}

impl std::ops::AddAssign<IAtomScaled> for LinearSum {
    fn add_assign(&mut self, rhs: IAtomScaled) {
        self.elements.push(rhs)
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
        let mut sum: Vec<_> = self
            .sum
            .elements
            .iter()
            .map(|e| NFLinearSumItem {
                factor: e.factor,
                var: e.item.into(),
            })
            .collect();
        sum.sort();
        // TODO: merge duplicates
        NormalExpr::Pos(NFLinearLeq {
            sum,
            upper_bound: self.ub,
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
