use crate::errors::ToEnvMessage;
use crate::utils::disp_slice;
use crate::*;
use Type::*;
use errors::{Message, Spanned};
use smallvec::{SmallVec, smallvec};
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
                expr.invalid(format!(
                    "has type `{}` but type `{}` was expected",
                    expr.tpe(),
                    expected
                ))
            }
            TypeError::UnexpectedArgument(expr) => {
                let expr = env / expr;
                expr.invalid("Unexpected argument")
            }
            TypeError::MissingParameter(param) => errors::Message::error(format!("missing parameter: {}", param)),
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

    pub fn top_user_type(&self) -> UserType {
        UserType::new(self.user_types.top_type.clone(), self.user_types.clone())
    }

    pub fn get_user_type(&self, name: impl Into<Sym>) -> Result<UserType, TypeError> {
        let name = name.into();
        self.check_type(&name)?;
        Ok(UserType::new(name, self.user_types.clone()))
    }

    pub fn get_user_type_or_top(&self, name: Option<impl Into<Sym>>) -> Result<UserType, TypeError> {
        if let Some(name) = name {
            self.get_user_type(name)
        } else {
            Ok(self.top_user_type())
        }
    }

    fn check_type(&self, name: &Sym) -> Result<(), TypeError> {
        if !self.user_types.contains(name.clone()) {
            Err(TypeError::UnknownType(name.clone()))
        } else {
            Ok(())
        }
    }

    pub fn get_union_type<'a, T>(&self, types: &'a [T]) -> Result<Type, TypeError>
    where
        &'a T: Into<Sym>,
    {
        let mut union = SmallVec::with_capacity(types.len());
        for t in types {
            let t = t.into();
            self.check_type(&t)?;
            union.push(t);
        }
        if types.is_empty() {
            Ok((&self.top_user_type()).into())
        } else {
            Ok(Type::User(UnionUserType {
                union,
                hier: self.user_types.clone(),
            }))
        }
    }
}

/// Represent a type as the union of possible user types.
#[derive(Clone)]
pub struct UnionUserType {
    union: SmallVec<[Sym; 1]>,
    hier: Arc<UserTypes>,
}

impl UnionUserType {
    pub fn new(tpe: impl Into<Sym>, hier: Arc<UserTypes>) -> Self {
        UnionUserType {
            union: smallvec![tpe.into()],
            hier,
        }
    }

    pub fn is_subtype_of(&self, other: &UnionUserType) -> bool {
        self.union
            .iter()
            .all(|t| other.union.iter().any(|t2| self.hier.is_subtype_of(t, t2)))
    }
}

impl Display for UnionUserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.union.len() == 1 {
            write!(f, "{}", self.union[0])
        } else {
            write!(f, "{{")?;
            disp_slice(f, self.union.as_slice(), ", ")?;
            write!(f, "}}")
        }
    }
}

/// Represents a single user-defined type within a a type hierarchy
#[derive(Clone)]
pub struct UserType {
    pub name: Sym,
    pub hier: Arc<UserTypes>,
}
impl UserType {
    fn new(name: Sym, hier: Arc<UserTypes>) -> Self {
        Self {
            name,
            hier: hier.clone(),
        }
    }
}
impl From<&UserType> for Type {
    fn from(value: &UserType) -> Self {
        Type::User(UnionUserType::new(value.name.clone(), value.hier.clone()))
    }
}
impl PartialEq for UserType {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
impl Debug for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
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
            top_type: Sym::from("★object★"),
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

    /// Records a new type with the given parent.
    /// If the parent is not recorded yet, it is created (asuming no parents)
    /// If the type already exists, a new parent is added (multiple inheritence)
    pub fn add_type<T: Into<Sym>>(&mut self, tpe: T, parent: Option<T>) {
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
    }
}

#[derive(Clone, Copy)]
pub struct IntInterval(Option<IntValue>, Option<IntValue>);

impl IntInterval {
    pub const FULL: IntInterval = IntInterval(None, None);
    pub fn singleton(value: IntValue) -> Self {
        Self(Some(value), Some(value))
    }

    /// Creates the interval [min, oo[
    pub fn at_least(min: IntValue) -> Self {
        Self(Some(min), None)
    }

    /// Creates the interval ]-oo, max]
    pub fn at_most(max: IntValue) -> Self {
        Self(None, Some(max))
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
    Real,
    User(UnionUserType),
}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        self.is_subtype_of(other) && other.is_subtype_of(self)
    }
}

impl Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bool => write!(f, "bool"),
            Int(_) => write!(f, "int"),
            Real => write!(f, "real"),
            User(name) => write!(f, "{name}"),
        }
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Type {
    /// Unbounded int type
    pub const INT: Type = Type::Int(IntInterval::FULL);

    /// Unbounded real type
    pub const REAL: Type = Type::Real;

    pub fn is_subtype_of(&self, other: &Type) -> bool {
        match (self, other) {
            (Bool, Bool) => true,
            (Real, Real) => true,
            (Int(bounds1), Int(bounds2)) => bounds1.is_subset_of(bounds2),
            (User(left), User(right)) => left.is_subtype_of(right),
            (Int(_), Real) => true,
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
