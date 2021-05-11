use crate::lang::Atom;

pub type Args = Vec<Atom>;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum Fun {
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
                Fun::Or => "or",
                Fun::Eq => "=",
                Fun::Leq => "<=",
                Fun::Max => "max",
                Fun::OptEq => "opt_eq",
                Fun::OptLeq => "opt_leq",
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
}
