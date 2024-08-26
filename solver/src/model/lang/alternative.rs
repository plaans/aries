use crate::core::state::Term;
use crate::core::{IntCst, VarRef};
use crate::model::lang::{Atom, ConversionError, IAtom};
use crate::reif::ReifExpr;
use itertools::Itertools;

pub struct Alternative {
    main: Atom,
    alternatives: Vec<Atom>,
}

impl Alternative {
    pub fn new<T: Into<Atom>>(main: impl Into<Atom>, alternatives: impl IntoIterator<Item = T>) -> Self {
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
        match value.main {
            Atom::Int(main) => {
                let alts: Vec<IAtom> = value
                    .alternatives
                    .iter()
                    .copied()
                    .map(IAtom::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                // main = main.var + main.shift = oneof(alts)
                // main.var = oneof(alts) - main.shift

                let alts = alts
                    .iter()
                    .map(|iatom| NFAlternativeItem {
                        var: iatom.var.variable(),
                        cst: iatom.shift - main.shift,
                    })
                    .sorted()
                    .collect_vec();

                Ok(NFAlternative {
                    main: main.var.variable(),
                    alternatives: alts,
                })
            }
            _ => todo!("Unsupported non-int alternative"),
        }
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Debug, Ord, PartialOrd)]
pub struct NFAlternativeItem {
    pub var: VarRef,
    pub cst: IntCst,
}
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct NFAlternative {
    pub main: VarRef,
    // sorted alternatives
    pub alternatives: Vec<NFAlternativeItem>,
}
