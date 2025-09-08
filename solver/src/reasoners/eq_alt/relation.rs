use std::{
    fmt::Display,
    ops::{Add, Not, Sub},
};
use EqRelation::*;

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
                Eq => "==",
                Neq => "!=",
            }
        )
    }
}

impl Add for EqRelation {
    type Output = Option<Self>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Eq, Eq) => Some(Eq),
            (Neq, Eq) => Some(Neq),
            (Eq, Neq) => Some(Neq),
            (Neq, Neq) => None,
        }
    }
}

impl Sub for EqRelation {
    type Output = Option<Self>;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Eq, Eq) => Some(Eq),
            (Eq, Neq) => None,
            (Neq, Eq) => Some(Neq),
            (Neq, Neq) => Some(Eq),
        }
    }
}

impl Not for EqRelation {
    type Output = EqRelation;

    fn not(self) -> Self::Output {
        match self {
            Eq => Neq,
            Neq => Eq,
        }
    }
}
