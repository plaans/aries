use crate::Fmt;
use itertools::Itertools;
use std::convert::TryFrom;
use std::fmt::Display;
use std::sync::Arc;

pub struct Input {
    pub text: String,
    pub source: Option<String>,
}

impl Input {
    pub fn from_string(input: impl Into<String>) -> Input {
        Input {
            text: input.into(),
            source: None,
        }
    }

    pub fn from_file(file: &std::path::Path) -> std::result::Result<Input, std::io::Error> {
        let s = std::fs::read_to_string(file)?;
        Ok(Input {
            text: s,
            source: Some(file.display().to_string()),
        })
    }

    fn indices(&self, span: Span) -> Option<(usize, usize)> {
        let mut start = None;
        let mut end = None;
        let mut line = 0;
        let mut column = 0;
        for (char, c) in self.text.chars().enumerate() {
            let pos = Pos { line, column };
            if pos == span.start {
                start = Some(char);
            }
            if pos == span.end {
                end = Some(char);
            }

            column += 1;
            if c == '\n' {
                line += 1;
                column = 0;
            }
        }
        match (start, end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }

    /// Returns the substring corresponding to this span.
    /// Panics if the span does not fits in the source text.
    pub fn substring(&self, span: Span) -> &str {
        let (start, end) = self.indices(span).expect("Invalid span");
        &self.text[start..=end]
    }

    pub fn underlined_position(&self, pos: Pos) -> impl std::fmt::Display + '_ {
        self.underlined(Span { start: pos, end: pos })
    }

    pub fn underlined(&self, span: Span) -> impl std::fmt::Display + '_ {
        let formatter = move |f: &mut std::fmt::Formatter| {
            let l = self
                .text
                .lines()
                .dropping(span.start.line as usize)
                .next()
                .expect("Invalid span for this source");
            assert!((span.start.column as usize) < l.len());
            writeln!(f, "{}", l)?;

            let num_spaces = span.start.column as usize;
            let length = if span.start.line != span.end.line {
                l.len() - (span.start.column as usize)
            } else {
                (span.end.column - span.start.column + 1) as usize
            };
            // print spaces in front of underline, attempting to have the same spacing by place tabulation
            // when their are some in the input.
            for c in l[0..num_spaces].chars() {
                let output = if c == '\t' { '\t' } else { ' ' };
                write!(f, "{}", output)?;
            }

            write!(f, "{}", "^".repeat(length))?;

            Ok(())
        };
        Fmt(formatter)
    }
}

impl From<&str> for Input {
    fn from(s: &str) -> Self {
        Input {
            text: s.to_string(),
            source: None,
        }
    }
}

impl TryFrom<&std::path::Path> for Input {
    type Error = std::io::Error;

    fn try_from(path: &std::path::Path) -> Result<Self, Self::Error> {
        Input::from_file(path)
    }
}

/// Position of a single character in an input.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Pos {
    pub line: u32,
    pub column: u32,
}

/// Part of an input, denoted by the start and end position, both inclusive.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub fn new(start: Pos, end: Pos) -> Span {
        Span { start, end }
    }
    pub fn point(position: Pos) -> Span {
        Span {
            start: position,
            end: position,
        }
    }
}

/// A slice of an input.
/// Mostly used to produce localized error messages through the `invalid` method.
#[derive(Clone)]
pub struct Loc {
    source: std::sync::Arc<Input>,
    span: Span,
}

impl Loc {
    pub fn new(source: &Arc<Input>, span: Span) -> Loc {
        Loc {
            source: source.clone(),
            span,
        }
    }

    pub fn invalid(self, error: impl Into<String>) -> ErrLoc {
        ErrLoc {
            context: vec![],
            inline_err: Some(error.into()),
            loc: Some(self),
        }
    }

    pub fn end(self) -> Loc {
        Loc {
            source: self.source,
            span: Span::new(self.span.end, self.span.end),
        }
    }

    pub fn span(self) -> Span {
        self.span
    }

    pub fn underlined(&self) -> impl Display + '_ {
        self.source.underlined(self.span)
    }
}

impl std::fmt::Debug for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source.substring(self.span))
    }
}

pub struct ErrLoc {
    context: Vec<String>,
    inline_err: Option<String>,
    loc: Option<Loc>,
}

impl ErrLoc {
    pub fn with_error(mut self, inline_message: impl Into<String>) -> ErrLoc {
        self.inline_err = Some(inline_message.into());
        self
    }

    pub fn failed<T>(self) -> std::result::Result<T, ErrLoc> {
        Err(self)
    }
}
impl From<String> for ErrLoc {
    fn from(e: String) -> Self {
        ErrLoc {
            context: vec![],
            inline_err: Some(e),
            loc: None,
        }
    }
}

impl std::error::Error for ErrLoc {}

impl std::fmt::Display for ErrLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, context) in self.context.iter().rev().enumerate() {
            let prefix = if i > 0 { "Caused by" } else { "Error" };
            writeln!(f, "{}: {}", prefix, context)?;
        }
        if let Some(Loc { source, span }) = &self.loc {
            if let Some(path) = &source.source {
                writeln!(f, "{}:{}:{}", path, span.start.line + 1, span.start.column)?;
            }
            write!(f, "{}", source.underlined(*span))?;
        }
        if let Some(err) = &self.inline_err {
            write!(f, " {}", err)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for ErrLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub trait Ctx<T> {
    fn ctx(self, error_context: impl Display) -> std::result::Result<T, ErrLoc>;
}
impl<T> Ctx<T> for std::result::Result<T, ErrLoc> {
    fn ctx(self, error_context: impl Display) -> Result<T, ErrLoc> {
        match self {
            Ok(x) => Ok(x),
            Err(mut e) => {
                e.context.push(format!("{}", error_context));
                Err(e)
            }
        }
    }
}

/// Wrapper around a String which can optionally provide the location it was defined at.
#[derive(Clone)]
pub struct Sym {
    /// Canonical version of the symbol, used for comparison (e.g. all lowercase for case-insensitive systems).
    pub canonical: String,
    /// When provided, used to display the symbol. Otherwise, the canonical field is used.
    display: Option<String>,
    /// Source of the symbol, typcally a span in a source file. Used for pretty printing errors messages.
    source: Option<Loc>,
}

impl Sym {
    pub fn new(s: impl Into<String>) -> Sym {
        Sym {
            canonical: s.into(),
            display: None,
            source: None,
        }
    }

    pub fn with_source(s: impl Into<String>, display: Option<String>, source: Loc) -> Sym {
        Sym {
            canonical: s.into(),
            display,
            source: Some(source),
        }
    }

    pub fn canonical_str(&self) -> &str {
        self.canonical.as_str()
    }

    pub fn canonical_string(&self) -> String {
        self.canonical.clone()
    }

    pub fn loc(&self) -> Loc {
        match &self.source {
            Some(loc) => loc.clone(),
            None => {
                let input = Input::from_string(&self.canonical);
                let span = Span {
                    start: Pos { line: 0, column: 0 },
                    end: Pos {
                        line: 0,
                        column: (self.canonical.len() - 1) as u32,
                    },
                };
                Loc {
                    source: Arc::new(input),
                    span,
                }
            }
        }
    }

    pub fn invalid(&self, error: impl Into<String>) -> ErrLoc {
        self.loc().invalid(error)
    }
}

impl std::cmp::PartialEq for Sym {
    fn eq(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }
}

impl std::cmp::Eq for Sym {}

impl std::cmp::PartialOrd for Sym {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Sym {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.canonical.cmp(&other.canonical)
    }
}

impl AsRef<str> for Sym {
    fn as_ref(&self) -> &str {
        &self.canonical
    }
}

impl std::borrow::Borrow<str> for Sym {
    fn borrow(&self) -> &str {
        &self.canonical
    }
}
impl std::borrow::Borrow<String> for Sym {
    fn borrow(&self) -> &String {
        &self.canonical
    }
}

impl std::hash::Hash for Sym {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical.hash(state)
    }
}

impl From<&str> for Sym {
    fn from(s: &str) -> Self {
        Sym {
            canonical: s.to_string(),
            display: None,
            source: None,
        }
    }
}
impl From<&String> for Sym {
    fn from(s: &String) -> Self {
        Sym {
            canonical: s.to_string(),
            display: None,
            source: None,
        }
    }
}
impl From<String> for Sym {
    fn from(s: String) -> Self {
        Sym {
            canonical: s,
            display: None,
            source: None,
        }
    }
}
impl From<&Sym> for Sym {
    fn from(s: &Sym) -> Self {
        s.clone()
    }
}
impl From<&Sym> for String {
    fn from(s: &Sym) -> Self {
        s.canonical.clone()
    }
}

impl std::fmt::Display for Sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            if let Some(d) = &self.display {
                d
            } else {
                &self.canonical
            }
        )
    }
}

impl std::fmt::Debug for Sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self)
    }
}
