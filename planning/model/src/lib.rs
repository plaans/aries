mod actions;
mod effects;
mod env;
pub mod errors;
mod expressions;
mod fluents;
mod goals;
mod metrics;
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
pub use env::*;
pub use expressions::*;
pub use fluents::*;
pub use goals::*;
pub use metrics::*;
pub use model::*;
pub use objects::*;
pub use params::*;
pub use timing::*;
pub use types::*;

use errors::{Span, Spanned};

pub type Res<T> = anyhow::Result<T>;

/// Symbol in the model, possibly annotate with its origin (file/line)
#[derive(Clone)]
pub struct Sym {
    /// Canonical view of the symbol (e.g. lower cased for PDDL)
    pub symbol: String,
    /// Origin of the symbol. If non-empty, can be used to derive the Display view of the symbol (e.g. properly capitalized)
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

impl From<&Sym> for Sym {
    fn from(value: &Sym) -> Self {
        value.clone()
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
        let view = if let Some(span) = self.span.as_ref() {
            span.str()
        } else {
            self.symbol.as_str()
        };
        write!(f, "{}", view)
    }
}

impl PartialEq for Sym {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for Sym {}

impl PartialOrd for Sym {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Sym {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.symbol.cmp(&other.symbol)
    }
}

impl Hash for Sym {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.symbol.hash(state)
    }
}
