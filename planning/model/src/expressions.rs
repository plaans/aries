use std::sync::Arc;

use derive_more::derive::Display;
use errors::Spanned;
use utils::disp_iter;

use crate::*;

#[derive(Clone, Display)]
#[display("{}", expr)]
pub struct TypedExpr {
    expr: Arc<Expr>,
    tpe: Type,
    span: Option<Span>,
}

impl Debug for TypedExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.expr)
    }
}

impl Spanned for TypedExpr {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

impl TypedExpr {
    pub fn tpe(&self) -> &Type {
        &self.tpe
    }
    pub fn expr(&self) -> &Expr {
        self.expr.as_ref()
    }
}

#[derive(Clone, Debug)]
pub enum Expr {
    Int(u64),
    Bool(bool),
    Object(Object),
    Param(Param),
    App(Fun, Vec<TypedExpr>),
    StateVariable(Fluent, Vec<TypedExpr>),
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Int(i) => write!(f, "{i}"),
            Expr::Bool(b) => write!(f, "{b}"),
            Expr::Object(o) => write!(f, "{o}"),
            Expr::Param(p) => write!(f, "{p}"),
            Expr::App(function, args) => {
                write!(f, "{}(", function)?;
                disp_iter(f, &args, ", ")?;
                write!(f, ")")
            }
            Expr::StateVariable(fluent, args) => {
                write!(f, "{}(", fluent.name())?;
                disp_iter(f, &args, ", ")?;
                write!(f, ")")
            }
        }
    }
}

impl Expr {
    pub fn tpe(&self) -> Result<Type, TypeError> {
        match self {
            Expr::Int(i) => Ok(Type::Int(IntInterval::singleton(*i))),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::App(fun, args) => fun.return_type(args.as_slice()),
            Expr::StateVariable(fluent, args) => fluent.return_type(args.as_slice()),
            Expr::Object(o) => Ok(o.tpe().clone()),
            Expr::Param(p) => Ok(p.tpe().clone()),
        }
    }

    pub fn typed(self, span: impl Into<Option<Span>>) -> Result<TypedExpr, TypeError> {
        let tpe = self.tpe()?;
        Ok(TypedExpr {
            expr: Arc::new(self),
            tpe,
            span: span.into(),
        })
    }
}

#[derive(Clone, Debug)]
pub enum Fun {
    Plus,
    Minus,
    And,
    Or,
    Not,
}

impl Fun {
    pub fn return_type(&self, args_types: &[TypedExpr]) -> Result<Type, TypeError> {
        // TODO: specialize for parameters
        use Fun::*;
        match self {
            Fun::Plus | Fun::Minus => {
                for a in args_types {
                    Type::INT.accepts(a)?;
                }
                Ok(Type::INT)
            }
            And | Or => {
                for a in args_types {
                    Type::Bool.accepts(a)?;
                }
                Ok(Type::Bool)
            }
            Not => {
                match args_types {
                    &[] => Err(TypeError::MissingParameter(Param::new("<negated-term>", Type::Bool))),
                    &[ref single] => {
                        Type::Bool.accepts(single)?;
                        Ok(Type::Bool)
                    }
                    &[_, ref second, ..] => Err(TypeError::UnexpectedArgument(second.clone())),
                }
                // if args_types.is_empty() {
                //     Err(TypeError::MissingParameter(Param::new("<negated-term>", Type::Bool)))
                // } else if args_types.len() >
            }
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
                Fun::And => "and",
                Fun::Or => "or",
                Fun::Not => "not",
            }
        )
    }
}
