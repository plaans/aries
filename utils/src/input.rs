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
            for c in (&l[0..num_spaces]).chars() {
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

impl std::fmt::Debug for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source.substring(self.span))
    }
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
