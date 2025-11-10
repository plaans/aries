use errors::Spanned;
use itertools::Itertools;
use num_rational::Rational64;
use smallvec::SmallVec;

use crate::{
    env::{Env, Environment},
    errors::{EnvError, ErrorMessageExt, Message},
    utils::disp_iter,
    *,
};

pub type IntValue = i64;
pub type RealValue = Rational64;

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone, Copy)]
pub struct ExprId(pub(crate) u32);

pub type SeqExprId = SmallVec<[ExprId; 3]>;

impl idmap::intid::IntegerId for ExprId {
    idmap::intid::impl_newtype_id_body!(for ExprId(u32));
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
    pub fn get(&self) -> &'a ExprNode {
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
    Real(RealValue),
    Bool(bool),
    Object(Object),
    Param(Param),
    App(Fun, SeqExprId),
    StateVariable(FluentId, SeqExprId),
    Exists(Vars, ExprId),
    Forall(Vars, ExprId),
    Instant(Timestamp),
    Duration,
    Makespan,
    ViolationCount(RefId),
}

pub type Vars = Vec<Param>;

impl<'a> Display for TExpr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.expr() {
            Expr::Real(i) if i.is_integer() => write!(f, "{}", i.numer()),
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
            Expr::Makespan => write!(f, "?makespan"),
            Expr::ViolationCount(ref_id) => write!(f, "violations({ref_id})"),
            Expr::Exists(params, expr_id) => {
                write!(f, "(exists {} {})", params.iter().join(", "), self.env / *expr_id)
            }
            Expr::Forall(params, expr_id) => {
                write!(f, "(forall {} {})", params.iter().join(", "), self.env / *expr_id)
            }
            Expr::Instant(tp) => write!(f, "{tp}"),
        }
    }
}

impl Expr {
    pub fn tpe(&self, env: &Environment) -> Result<Type, Message> {
        match self {
            Expr::Real(i) if i.is_integer() => Ok(Type::Int(IntInterval::singleton(*i.numer()))),
            Expr::Real(_) => Ok(Type::Real),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::App(fun, args) => fun.return_type(args.as_slice(), env).msg(env),
            Expr::StateVariable(fluent, args) => {
                let fluent = env.fluents.get(*fluent);
                fluent
                    .return_type(args.as_slice(), env)
                    .msg(env)
                    .tag(fluent, "fluent declaration", None)
            }
            Expr::Object(o) => Ok(o.tpe().into()),
            Expr::Param(p) => Ok(p.tpe().clone()),
            Expr::Exists(_, x) | Expr::Forall(_, x) => Ok(env.node(*x).tpe().clone()),
            Expr::Duration | Expr::Makespan => Ok(Type::Real),
            Expr::ViolationCount(_) => Ok(Type::Int(IntInterval::at_least(0))),
            Expr::Instant(_) => Ok(Type::Real),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fun {
    Plus,
    Minus,
    Div,
    Mul,
    And,
    Or,
    Implies,
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
            And | Or | Implies => {
                if self == &Fun::Implies && args_types.len() > 2 {
                    return Err(TypeError::UnexpectedArgument(args_types[2]));
                }
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
                Fun::Implies => "implies",
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
