use crate::core::state::Term;
use crate::core::{IntCst, SignedVar};
use crate::model::lang::IAtom;
use crate::reif::ReifExpr;
use itertools::Itertools;
use std::fmt::{Debug, Formatter};

/// Constraint equivalent to `lhs = max { e | e \in rhs }`
pub struct EqMax {
    lhs: IAtom,
    rhs: Vec<IAtom>,
}

impl EqMax {
    pub fn new<T: Into<IAtom>>(max: impl Into<IAtom>, elements: impl IntoIterator<Item = T>) -> Self {
        Self {
            lhs: max.into(),
            rhs: elements.into_iter().map(|e| e.into()).collect_vec(),
        }
    }
}

/// Constraint equivalent to `lhs = min { e | e \in rhs }`
pub struct EqMin {
    lhs: IAtom,
    rhs: Vec<IAtom>,
}

impl EqMin {
    pub fn new<T: Into<IAtom>>(min: impl Into<IAtom>, elements: impl IntoIterator<Item = T>) -> Self {
        Self {
            lhs: min.into(),
            rhs: elements.into_iter().map(|e| e.into()).collect_vec(),
        }
    }
}

impl From<EqMax> for ReifExpr {
    fn from(em: EqMax) -> Self {
        // (lhs_var + lhs_cst)  =  max_i  {  var_i + cst_i }
        // (lhs_var)  =  max_i  {  var_i + cst_i  - lhs_cst}
        let lhs_var = SignedVar::plus(em.lhs.var.variable());
        let lhs_cst = em.lhs.shift;
        let rhs = em
            .rhs
            .iter()
            .map(|term| NFEqMaxItem {
                var: SignedVar::plus(term.var.variable()),
                cst: term.shift - lhs_cst,
            })
            .sorted()
            .collect_vec();

        ReifExpr::EqMax(NFEqMax { lhs: lhs_var, rhs })
    }
}

impl From<EqMin> for ReifExpr {
    fn from(em: EqMin) -> Self {
        // (lhs_var + lhs_cst)  =    min_i  {    var_i + cst_i }
        // (lhs_var + lhs_cst)  =  - max_i  {  - var_i - cst_i }
        // - lhs_var - lhs_cst  =    max_i  {  - var_i - cst_i }
        // - lhs_var            =    max_i  {  - var_i - cst_i + lhs_cst}
        let lhs_var = em.lhs.var.variable();
        let lhs = SignedVar::minus(lhs_var);
        let lhs_cst = em.lhs.shift;
        let rhs = em
            .rhs
            .iter()
            .map(|term| NFEqMaxItem {
                var: SignedVar::minus(term.var.variable()),
                cst: -term.shift + lhs_cst,
            })
            .sorted()
            .collect_vec();

        ReifExpr::EqMax(NFEqMax { lhs, rhs })
    }
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct NFEqMax {
    pub lhs: SignedVar,
    pub rhs: Vec<NFEqMaxItem>,
}

impl Debug for NFEqMax {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = max ", self.lhs)?;
        f.debug_set().entries(self.rhs.iter()).finish()
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Clone)]
pub struct NFEqMaxItem {
    pub var: SignedVar,
    pub cst: IntCst,
}

#[allow(clippy::comparison_chain)]
impl Debug for NFEqMaxItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.var)?;
        if self.cst > 0 {
            write!(f, " + {}", self.cst)?;
        } else if self.cst < 0 {
            write!(f, " - {}", -self.cst)?;
        }
        Ok(())
    }
}
