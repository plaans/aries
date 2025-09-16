use crate::errors::{Span, Spanned};
use std::fmt::{Debug, Display};

/// Symbol in the model, possibly annotate with its origin (file/line)
#[derive(Clone)]
pub struct Sym {
    /// Canonical view of the symbol (e.g. lower cased for PDDL)
    pub symbol: String,
    /// Origin of the symbol. If non-empty, can be used to derive the Display view of the symbol (e.g. properly capitalized)
    pub span: Option<Span>,
}

impl Sym {
    pub fn with_source(s: impl Into<String>, source: Span) -> Sym {
        Sym {
            symbol: s.into(),
            span: Some(source),
        }
    }

    pub fn canonical_str(&self) -> &str {
        self.symbol.as_str()
    }

    pub fn canonical_string(&self) -> String {
        self.symbol.clone()
    }
}

impl AsRef<str> for Sym {
    fn as_ref(&self) -> &str {
        &self.symbol
    }
}

impl std::borrow::Borrow<str> for Sym {
    fn borrow(&self) -> &str {
        &self.symbol
    }
}
impl std::borrow::Borrow<String> for Sym {
    fn borrow(&self) -> &String {
        &self.symbol
    }
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

impl std::hash::Hash for Sym {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.symbol.hash(state)
    }
}
