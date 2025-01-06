mod actions;
mod effects;
pub mod errors;
mod expressions;
mod fluents;
mod model;
mod objects;
mod params;
pub mod pddl;
mod timing;
mod types;
pub(crate) mod utils;

use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

pub use actions::*;
pub use effects::*;
pub use expressions::*;
pub use fluents::*;
pub use model::*;
pub use objects::*;
pub use params::*;
pub use timing::*;
pub use types::*;

use errors::{Span, Spanned};

pub type Res<T> = anyhow::Result<T>;

#[derive(Clone)]
pub struct Sym {
    pub symbol: String,
    pub span: Option<Span>,
}

impl From<&str> for Sym {
    fn from(value: &str) -> Self {
        Sym {
            symbol: value.to_string(),
            span: None,
        }
    }
}

impl Spanned for Sym {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

impl Debug for Sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol)
    }
}
impl Display for Sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

impl PartialEq for Sym {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for Sym {}

impl Hash for Sym {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.symbol.hash(state)
    }
}
