use crate::errors::ToEnvMessage;
use crate::*;
use Type::*;
use errors::{Message, Spanned};
use std::fmt::Debug;
use std::{ops::RangeInclusive, sync::Arc};
use thiserror::Error;

#[derive(Debug)]
pub enum TypeError {
    UnknownType(Sym),
    IncompatibleType(ExprId, Type),
    MissingParameter(Param),
    UnexpectedArgument(ExprId),
}

impl ToEnvMessage for TypeError {
    fn to_message(self, env: &Environment) -> Message {
        match self {
            TypeError::UnknownType(_) => todo!(),
            TypeError::IncompatibleType(expr, expected) => {
                let expr = env / expr;
                errors::Message::error("Incompatible types").snippet(expr.error(format!(
                    "has type `{}` but type `{}` was expected",
                    expr.tpe(),
                    expected
                )))
            }
            _ => todo!(),
        }
    }
}

#[derive(Error, Debug)]
pub enum UserTypeDeclarationError {}

pub struct Types {
    user_types: Arc<UserTypes>,
}

impl Types {
    pub fn new(types: UserTypes) -> Self {
        Self {
            user_types: Arc::new(types),
        }
    }

    pub fn top_user_type(&self) -> Type {
        Type::User(self.user_types.top_type.clone(), self.user_types.clone())
    }

    pub fn get_user_type(&self, name: impl Into<Sym>) -> Result<Type, TypeError> {
        let name = name.into();
        if self.user_types.contains(name.clone()) {
            Ok(Type::User(name, self.user_types.clone()))
        } else {
            Err(TypeError::UnknownType(name))
        }
    }

    pub fn get_user_type_or_top(&self, name: Option<impl Into<Sym>>) -> Result<Type, TypeError> {
        if let Some(name) = name {
            self.get_user_type(name)
        } else {
            Ok(self.top_user_type())
        }
    }
}

#[derive(Clone)]
pub struct UserTypes {
    top_type: Sym,
    types: hashbrown::HashMap<Sym, Vec<Sym>>,
}

impl Default for UserTypes {
    fn default() -> Self {
        Self::new()
    }
}

impl UserTypes {
    pub fn new() -> Self {
        Self {
            top_type: Sym {
                symbol: "★object★".to_string(),
                span: None,
            },
            types: Default::default(),
        }
    }

    pub fn is_subtype_of(&self, a: &Sym, b: &Sym) -> bool {
        if a == b {
            true
        } else if let Some(parents) = self.types.get(a) {
            parents.iter().any(|parent| self.is_subtype_of(parent, b))
        } else {
            false
        }
    }

    pub fn contains(&self, name: impl Into<Sym>) -> bool {
        let name = name.into();
        self.types.contains_key(&name)
    }

    pub fn add_type<T: Into<Sym>>(&mut self, tpe: T, parent: Option<T>) -> Result<(), UserTypeDeclarationError> {
        let tpe = tpe.into();
        let parent = parent.map(|p| p.into());
        if let Some(parent) = parent {
            if !self.types.contains_key(&parent) {
                self.types.insert(parent.clone(), Vec::new());
            }
            self.types.entry(tpe).or_default().push(parent);
        } else {
            self.types.entry(tpe).or_default();
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct IntInterval(Option<IntValue>, Option<IntValue>);

impl IntInterval {
    pub const FULL: IntInterval = IntInterval(None, None);
    pub fn singleton(value: IntValue) -> Self {
        Self(Some(value), Some(value))
    }

    pub fn is_subset_of(&self, other: &IntInterval) -> bool {
        other.is_superset_of(self)
    }
    pub fn is_superset_of(&self, other: &IntInterval) -> bool {
        let left_ok = match (self.0, other.0) {
            (None, _) => true,
            (Some(l), Some(r)) => l <= r,
            _ => false,
        };
        let right_ok = match (self.1, other.1) {
            (None, _) => true,
            (Some(l), Some(r)) => l >= r,
            _ => false,
        };
        left_ok && right_ok
    }
}

impl From<RangeInclusive<IntValue>> for IntInterval {
    fn from(value: RangeInclusive<IntValue>) -> Self {
        IntInterval(Some(*value.start()), Some(*value.end()))
    }
}

#[derive(Clone)]
pub enum Type {
    Bool,
    Int(IntInterval),
    User(Sym, Arc<UserTypes>),
}

impl Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bool => write!(f, "bool"),
            Int(_) => write!(f, "int"),
            User(name, _) => write!(f, "{name}"),
        }
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Type {
    pub const INT: Type = Type::Int(IntInterval::FULL);

    pub fn is_subtype_of(&self, other: &Type) -> bool {
        match (self, other) {
            (Bool, Bool) => true,
            (Int(bounds1), Int(bounds2)) => bounds1.is_subset_of(bounds2),
            (User(left, types), User(right, _)) => types.is_subtype_of(left, right),
            _ => false,
        }
    }

    pub fn accepts(&self, expr: ExprId, env: &Environment) -> Result<(), TypeError> {
        if env.node(expr).tpe().is_subtype_of(self) {
            Ok(())
        } else {
            Err(TypeError::IncompatibleType(expr, self.clone()))
        }
    }

    /// Returns true if two types are overlapping
    pub fn overlaps(&self, other: &Type) -> bool {
        self.is_subtype_of(other) || other.is_subtype_of(self)
    }
}
