use std::fmt::Debug;

use derive_more::derive::Display;

use crate::{types::Type, Sym};

#[derive(Clone, Display)]
#[display("{name}")]
pub struct Param {
    pub name: Sym,
    pub tpe: Type,
}

impl Debug for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.tpe)
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
