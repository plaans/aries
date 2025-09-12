use errors::Spanned;
use idmap::IntegerId;
use num_rational::Rational64;
use smallvec::SmallVec;

use crate::{
    env::{Env, Environment},
    errors::Message,
    utils::disp_iter,
    *,
};

pub type IntValue = i64;
pub type RealValue = Rational64;

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone, Copy)]
pub struct ExprId(pub(crate) u32);

pub type SeqExprId = SmallVec<[ExprId; 3]>;

impl IntegerId for ExprId {
    fn from_id(id: u64) -> Self {
        assert!(id <= (u32::MAX as u64));
        ExprId(id as u32)
    }

    fn id(&self) -> u64 {
        self.0 as u64
    }

    fn id32(&self) -> u32 {
        self.0
    }
}

pub(crate) struct ExprNode {
    expr: Expr,
    tpe: Type,
    span: Option<Span>,
}

impl ExprNode {
    pub fn new(expr: Expr, tpe: Type, span: Option<Span>) -> Self {
        Self { expr, tpe, span }
    }
}

pub type TExpr<'env> = Env<'env, ExprId>;

impl<'a> TExpr<'a> {
    fn get(&self) -> &'a ExprNode {
        self.env.get(self.elem)
    }
    pub fn bool(&self) -> Result<bool, Message> {
        if let Expr::Bool(value) = &self.get().expr {
            Ok(*value)
        } else {
            Err(Message::error("expected boolean value").snippet(self.error("not a boolean")))
        }
    }
    pub fn state_variable(&self) -> Result<(FluentId, &'a [ExprId]), Message> {
        if let Expr::StateVariable(fun, args) = &self.get().expr {
            Ok((*fun, args.as_slice()))
        } else {
            Err(Message::error("expected state variable value").snippet(self.error("not a state variable")))
        }
    }

    pub fn tpe(&self) -> &Type {
        &self.get().tpe
    }
    pub fn expr(&self) -> &Expr {
        &self.get().expr
    }
}

impl<'a> Debug for TExpr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl<'a> Spanned for TExpr<'a> {
    fn span(&self) -> Option<&Span> {
        self.get().span.as_ref()
    }
}
#[derive(Clone, Debug)]
pub enum Expr {
    Int(IntValue),
    Real(RealValue),
    Bool(bool),
    Object(Object),
    Param(Param),
    App(Fun, SeqExprId),
    StateVariable(FluentId, SeqExprId),
    Duration,
}

impl<'a> Display for TExpr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.expr() {
            Expr::Int(i) => write!(f, "{i}"),
            Expr::Real(r) => write!(f, "{r}"),
            Expr::Bool(b) => write!(f, "{b}"),
            Expr::Object(o) => write!(f, "{o}"),
            Expr::Param(p) => write!(f, "{p}"),
            Expr::App(function, args) => {
                write!(f, "{}(", function)?;
                disp_iter(f, args.iter().map(|&e| self.env / e), ", ")?;
                write!(f, ")")
            }
            Expr::StateVariable(fluent, args) => {
                write!(f, "{}(", (self.env / *fluent).name())?;
                disp_iter(f, args.iter().map(|&e| self.env / e), ", ")?;
                write!(f, ")")
            }
            Expr::Duration => write!(f, "?duration"),
        }
    }
}

impl Expr {
    pub fn tpe(&self, env: &Environment) -> Result<Type, TypeError> {
        match self {
            Expr::Int(i) => Ok(Type::Int(IntInterval::singleton(*i))),
            Expr::Real(_) => Ok(Type::Real),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::App(fun, args) => fun.return_type(args.as_slice(), env),
            Expr::StateVariable(fluent, args) => env.fluents.get(*fluent).return_type(args.as_slice(), env),
            Expr::Object(o) => Ok(o.tpe().clone()),
            Expr::Param(p) => Ok(p.tpe().clone()),
            Expr::Duration => Ok(Type::Real),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Fun {
    Plus,
    Minus,
    Div,
    Mul,
    And,
    Or,
    Not,
    Eq,
    Leq,
    Geq,
    Lt,
    Gt,
}

impl Fun {
    pub fn return_type(&self, args_types: &[ExprId], env: &Environment) -> Result<Type, TypeError> {
        // TODO: specialize for parameters
        use Fun::*;
        match self {
            Fun::Plus | Fun::Minus | Fun::Mul => {
                let mut is_int = true;
                for a in args_types {
                    if !(env / *a).tpe().is_subtype_of(&Type::INT) {
                        is_int = false;
                    }
                    Type::Real.accepts(*a, env)?;
                }
                if is_int { Ok(Type::INT) } else { Ok(Type::Real) }
            }
            Fun::Div => {
                for a in args_types {
                    Type::Real.accepts(*a, env)?;
                }
                Ok(Type::Real)
            }
            And | Or => {
                for a in args_types {
                    Type::Bool.accepts(*a, env)?;
                }
                Ok(Type::Bool)
            }
            Not => {
                match args_types {
                    [] => Err(TypeError::MissingParameter(Param::new("<negated-term>", Type::Bool))),
                    [single] => {
                        Type::Bool.accepts(*single, env)?;
                        Ok(Type::Bool)
                    }
                    [_, second, ..] => Err(TypeError::UnexpectedArgument(*second)),
                }
                // if args_types.is_empty() {
                //     Err(TypeError::MissingParameter(Param::new("<negated-term>", Type::Bool)))
                // } else if args_types.len() >
            }
            // binary operator
            Eq | Leq | Geq | Lt | Gt => match args_types {
                &[] | &[_] => Err(TypeError::MissingParameter(Param::new("<compared-term>", Type::Bool))),
                &[first, second] => {
                    match self {
                        Eq => Ok(Type::Bool), // do not enforce coherent typing for equality
                        Leq | Geq | Gt | Lt => {
                            Type::Real.accepts(first, env)?;
                            Type::Real.accepts(second, env)?;
                            Ok(Type::Bool)
                        }
                        _ => unreachable!(),
                    }
                }
                &[_, _, third, ..] => Err(TypeError::UnexpectedArgument(third)),
            },
        }
    }
}

impl Display for Fun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Fun::Plus => "+",
                Fun::Minus => "-",
                Fun::Div => "/",
                Fun::Mul => "*",
                Fun::And => "and",
                Fun::Or => "or",
                Fun::Not => "not",
                Fun::Eq => "=",
                Fun::Leq => "<=",
                Fun::Geq => ">=",
                Fun::Lt => "<",
                Fun::Gt => ">",
            }
        )
    }
}
