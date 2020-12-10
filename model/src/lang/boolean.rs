use crate::lang::BVar;
use std::cmp::Ordering;

// equivalent to lit
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct BAtom {
    pub var: Option<BVar>,
    pub negated: bool,
}
impl BAtom {
    pub fn new(var: Option<BVar>, negated: bool) -> BAtom {
        BAtom { var, negated }
    }

    pub fn lexical_cmp(&self, other: &BAtom) -> Ordering {
        match (self.var, other.var) {
            (Some(v1), Some(v2)) if v1 != v2 => v1.cmp(&v2),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            _ => self.negated.cmp(&other.negated),
        }
    }
}

impl std::ops::Not for BAtom {
    type Output = BAtom;

    fn not(self) -> Self::Output {
        BAtom::new(self.var, !self.negated)
    }
}

impl From<bool> for BAtom {
    fn from(value: bool) -> Self {
        BAtom {
            var: None,
            negated: !value,
        }
    }
}

impl From<BVar> for BAtom {
    fn from(b: BVar) -> Self {
        BAtom::new(Some(b), false)
    }
}
