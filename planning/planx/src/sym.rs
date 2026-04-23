use compact_str::CompactString;

use crate::errors::{Span, Spanned};
use std::{
    borrow::Cow,
    fmt::{Debug, Display},
};

/// Symbol in the model, possibly annotated with its origin (file/line).
///
/// A symbol is *case-insensitive* all representation will use a canonical (lower-case) form.
/// In particular all comparisons are made with respect to the case
/// The only way to get the original (case-sensitive) form is with the [`Sym::non_canonical_str`] method.
/// This default make sure that we do not mistakenly hand out a non-normalized view of the symbol.
///
/// Important: for convenience, the type implements equality comparison with `str` but comparing against a non-normalized `str` is
/// a logic error and will panic when debug assertions are enabled.
#[derive(Clone)]
pub struct Sym {
    /// Canonical view of the symbol (e.g. lower cased for PDDL)
    /// The underlying type uses small string optimization to avoid head allocation for short identifiers
    symbol: compact_str::CompactString,
    /// Origin of the symbol. If non-empty, can be used to derive the Display view of the symbol (e.g. properly capitalized)
    pub span: Option<Span>,
}

impl Sym {
    pub fn with_source<'a>(s: impl Into<Cow<'a, str>>, source: Span) -> Sym {
        let x: Cow<'a, str> = s.into();

        Sym {
            symbol: compact_str::CompactString::from_str_to_lowercase(&x),
            span: Some(source),
        }
    }

    pub fn as_str(&self) -> &str {
        self.canonical_str()
    }
    fn canonical_str(&self) -> &str {
        self.symbol.as_str()
    }

    pub fn non_canonical_str(&self) -> &str {
        if let Some(span) = self.span.as_ref() {
            span.str()
        } else {
            self.symbol.as_str()
        }
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
impl std::borrow::Borrow<str> for &Sym {
    fn borrow(&self) -> &str {
        &self.symbol
    }
}

impl From<&str> for Sym {
    fn from(value: &str) -> Self {
        Sym {
            symbol: CompactString::from_str_to_lowercase(value),
            span: None,
        }
    }
}

impl From<&Sym> for Sym {
    fn from(value: &Sym) -> Self {
        value.clone()
    }
}

impl From<Sym> for String {
    fn from(value: Sym) -> Self {
        value.canonical_str().to_string()
    }
}
impl From<&Sym> for String {
    fn from(value: &Sym) -> Self {
        value.canonical_str().to_string()
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
        write!(f, "{}", self.canonical_str())
    }
}

impl PartialEq for Sym {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_str() == other.canonical_str()
    }
}

impl PartialEq<str> for Sym {
    fn eq(&self, other: &str) -> bool {
        debug_assert_eq!(other, &other.to_lowercase(), "Non normalized string for comparison");
        self.canonical_str() == other
    }
}
impl PartialEq<Sym> for str {
    fn eq(&self, other: &Sym) -> bool {
        other == self
    }
}
impl PartialEq<&str> for Sym {
    fn eq(&self, other: &&str) -> bool {
        self == *other
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
