use crate::int_model::ILit;
use crate::lang::{Atom, IntCst, VarRef, Variable};
use std::convert::{TryFrom, TryInto};

pub type Args = Vec<Atom>;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum Fun {
    Or,
    Eq,
    Leq,
    Max,
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
        let mut args = Args::new();
        args.push(arg1.into());
        args.push(arg2.into());
        Expr { fun, args }
    }

    // TODO: remove, we should instead never be in the situation whete an expr can be directly encoded as a literal
    pub fn as_ilit(&self) -> Option<ILit> {
        if self.fun != Fun::Leq {
            return None;
        }
        if self.args.len() != 2 {
            return None;
        }
        let lhs: Variable = self.args[0].try_into().ok()?;
        let rhs: IntCst = self.args[1].try_into().ok()?;

        Some(ILit::leq(lhs, rhs))
    }
}
