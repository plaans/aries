use crate::bounds::Lit;
use crate::lang::{Atom, ConversionError};
use lazy_static::lazy_static;
use std::convert::TryFrom;

pub type Args = Vec<Atom>;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum Fun {
    /// Equal to its first argument
    Single,
    /// Negation of its first argument
    Not,
    Or,
    Eq,
    Leq,
    Max,
    /// Equality between two optional variables: if both are present, then they must be equal.
    OptEq,
    /// Inequality that must hold if both (optional) variables are present.
    OptLeq,
}

impl std::fmt::Display for Fun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Fun::Not => "not",
                Fun::Or => "or",
                Fun::Eq => "=",
                Fun::Leq => "<=",
                Fun::Max => "max",
                Fun::OptEq => "opt_eq",
                Fun::OptLeq => "opt_leq",
                Fun::Single => "head",
            }
        )
    }
}
impl std::fmt::Debug for Fun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct Expr {
    pub fun: Fun,
    pub args: Args,
}
impl Expr {
    pub fn new(fun: Fun, args: Args) -> Expr {
        Expr { fun, args }
    }

    pub fn new2(fun: Fun, arg1: impl Into<Atom>, arg2: impl Into<Atom>) -> Expr {
        let args = vec![arg1.into(), arg2.into()];
        Expr { fun, args }
    }

    #[allow(non_snake_case)]
    pub fn TRUE() -> Expr {
        EXPR_TRUE.clone()
    }
    #[allow(non_snake_case)]
    pub fn FALSE() -> Expr {
        EXPR_FALSE.clone()
    }

    pub fn lit(l: impl Into<Lit>) -> Expr {
        Expr::new(Fun::Single, vec![Atom::Bool(l.into())])
    }
}

impl From<Lit> for Expr {
    fn from(l: Lit) -> Self {
        Expr::lit(l)
    }
}
impl From<bool> for Expr {
    fn from(l: bool) -> Self {
        Expr::lit(l)
    }
}
impl TryFrom<&Expr> for Lit {
    type Error = ConversionError;

    fn try_from(value: &Expr) -> Result<Self, Self::Error> {
        match value.fun {
            Fun::Single => Lit::try_from(value.args[0]),
            Fun::Not => Ok(!Lit::try_from(value.args[0])?),
            _ => Err(ConversionError::NotLiteral),
        }
    }
}

lazy_static! {
    static ref EXPR_TRUE: Expr = Expr::lit(Lit::TRUE);
    static ref EXPR_FALSE: Expr = Expr::lit(Lit::FALSE);
}
