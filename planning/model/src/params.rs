use std::fmt::Debug;

use derive_more::derive::Display;

use crate::{Env, Sym, types::Type};

#[derive(Clone, Display)]
#[display("{name}")]
pub struct Param {
    pub name: Sym,
    pub tpe: Type,
}
impl PartialOrd for Param {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name().partial_cmp(other.name())
    }
}
impl Ord for Param {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name().cmp(other.name())
    }
}
impl PartialEq for Param {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Param {}

impl Debug for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.tpe)
    }
}

impl std::fmt::Display for Env<'_, &Param> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.elem)
    }
}

impl Param {
    pub fn new(name: impl Into<Sym>, tpe: Type) -> Self {
        Self { name: name.into(), tpe }
    }

    pub fn name(&self) -> &Sym {
        &self.name
    }
    pub fn tpe(&self) -> &Type {
        &self.tpe
    }
}
