use crate::Fmt;
use itertools::Itertools;
use std::convert::TryFrom;

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

            let num_spaces = span.start.column;
            let length = if span.start.line != span.end.line {
                l.len() - (span.start.column as usize)
            } else {
                (span.end.column - span.start.column + 1) as usize
            };

            write!(f, "{}{}", " ".repeat(num_spaces as usize), "^".repeat(length))?;

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
