use crate::{
    core::{Lit, Var},
    lang::{BoolExpr, Store},
    prelude::{Conjunction, DomainsExt, implies},
    reasoners::cp::mul::MulPropagator,
    reif::ReifExpr,
};
use std::fmt::{Debug, Formatter};

/// Represents the constraint  `lhs = rhs1 * rhs2`
#[derive(Eq, PartialEq, Hash, Clone)]
pub struct EqMul {
    lhs: Var,
    rhs1: Var,
    rhs2: Var,
}

impl EqMul {
    pub fn new(lhs: impl Into<Var>, factor1: impl Into<Var>, factor2: impl Into<Var>) -> Self {
        let factor1 = factor1.into();
        let factor2 = factor2.into();
        let (rhs1, rhs2) = if factor1 <= factor2 {
            (factor1, factor2)
        } else {
            (factor2, factor1)
        };
        Self {
            lhs: lhs.into(),
            rhs1,
            rhs2,
        }
    }
}

impl<Ctx: Store> BoolExpr<Ctx> for EqMul {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        let valid = ctx.presence_literal(implicant);

        for var in [self.lhs, self.rhs1, self.rhs2] {
            ctx.add_assertion(implies(valid, ctx.presence_literal(var)));
        }

        let propagator = MulPropagator {
            prod: self.lhs,
            fact1: self.rhs1,
            fact2: self.rhs2,
            active: implicant,
            valid,
        };
        ctx.enforce_user_propagator(propagator);
    }

    fn conj_scope(&self, ctx: &Ctx) -> crate::prelude::Conjunction {
        Conjunction::from([
            ctx.presence_literal(self.lhs),
            ctx.presence_literal(self.rhs1),
            ctx.presence_literal(self.rhs2),
        ])
    }
}

impl Debug for EqMul {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.rhs1, self.rhs2)
    }
}

#[derive(Clone)]
pub struct EqVarMulLit {
    pub lhs: Var,
    pub rhs: Var,
    pub lit: Lit,
}

impl Debug for EqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}

impl EqVarMulLit {
    pub fn new(lhs: impl Into<Var>, rhs: impl Into<Var>, lit: impl Into<Lit>) -> Self {
        let lhs = lhs.into();
        let rhs = rhs.into();
        let lit = lit.into();
        Self { lhs, rhs, lit }
    }
}

impl From<EqVarMulLit> for ReifExpr {
    fn from(value: EqVarMulLit) -> Self {
        ReifExpr::EqVarMulLit(NFEqVarMulLit {
            lhs: value.lhs,
            rhs: value.rhs,
            lit: value.lit,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct NFEqVarMulLit {
    pub lhs: Var,
    pub rhs: Var,
    pub lit: Lit,
}

impl Debug for NFEqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}
