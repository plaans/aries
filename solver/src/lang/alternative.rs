use crate::core::views::Term;
use crate::core::{IntCst, Var};
use crate::lang::{ConversionError, IAtom};
use crate::reif::ReifExpr;
use itertools::Itertools;

#[derive(Clone)]
pub struct Alternative {
    main: IAtom,
    alternatives: Vec<IAtom>,
}

impl Alternative {
    pub fn new<T: Into<IAtom>>(main: impl Into<IAtom>, alternatives: impl IntoIterator<Item = T>) -> Self {
        Self {
            main: main.into(),
            alternatives: alternatives.into_iter().map(|a| a.into()).collect_vec(),
        }
    }
}

impl From<Alternative> for ReifExpr {
    fn from(value: Alternative) -> Self {
        ReifExpr::Alternative(value.try_into().unwrap())
    }
}

impl TryFrom<Alternative> for NFAlternative {
    type Error = ConversionError;

    fn try_from(value: Alternative) -> Result<Self, Self::Error> {
        // main = main.var + main.shift = oneof(alts)
        // main.var = oneof(alts) - main.shift

        let alts = value
            .alternatives
            .iter()
            .map(|iatom| NFAlternativeItem {
                var: iatom.var.variable(),
                cst: iatom.shift - value.main.shift,
            })
            .sorted()
            .collect_vec();

        Ok(NFAlternative {
            main: value.main.var.variable(),
            alternatives: alts,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Debug, Ord, PartialOrd)]
pub struct NFAlternativeItem {
    pub var: Var,
    pub cst: IntCst,
}
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct NFAlternative {
    pub main: Var,
    // sorted alternatives
    pub alternatives: Vec<NFAlternativeItem>,
}
