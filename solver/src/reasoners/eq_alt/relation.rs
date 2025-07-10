use std::{fmt::Display, ops::Add};

/// Represents a eq or neq relationship between two variables.
/// Option\<EqRelation> should be used to represent a relationship between any two vars
///
/// Use + to combine two relationships. eq + neq = Some(neq), neq + neq = None
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum EqRelation {
    Eq,
    Neq,
}

impl Display for EqRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                EqRelation::Eq => "==",
                EqRelation::Neq => "!=",
            }
        )
    }
}

impl Add for EqRelation {
    type Output = Option<Self>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (EqRelation::Eq, EqRelation::Eq) => Some(EqRelation::Eq),
            (EqRelation::Neq, EqRelation::Eq) => Some(EqRelation::Neq),
            (EqRelation::Eq, EqRelation::Neq) => Some(EqRelation::Neq),
            (EqRelation::Neq, EqRelation::Neq) => None,
        }
    }
}
