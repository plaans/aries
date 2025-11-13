use std::{convert::TryFrom, hash::Hash};

use arcstr::ArcStr;

/// Represents an input string, typically from a file but designed to also accomate in memory strings.
/// When available, store the source file path which is used to print in-situ error messages.
///
/// Internally uses shared reference an thus cheaply cloneable.
/// 
/// Input are only considered equals
///
/// TODO: make the text inside a shared reference so we do not have to wrap the `Input` in Arc.
#[derive(Eq, Clone)]
pub struct Input {
    /// Text of the input
    pub(crate) text: ArcStr,
    /// Identifier of the source (typically the path to the file)
    /// This one is use to indicate the source in error outputs.
    pub(crate) source: Option<ArcStr>,
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        match (&self.source, &other.source) {
            (Some(x), Some(y)) => x == y && self.text == other.text,
            _ => false,
        }
    }
}

impl Hash for Input {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl Input {
    /// Creates a new input from the string.
    pub fn from_string(input: impl ToString) -> Input {
        Input {
            text: input.to_string().into(),
            source: None,
        }
    }

    /// Creates a new Input from the content of the file. The file name is stored as metadata of the `Input`
    pub fn from_file(file: &std::path::Path) -> std::result::Result<Input, std::io::Error> {
        let s = std::fs::read_to_string(file)?;
        Ok(Input {
            text: s.into(),
            source: Some(file.display().to_string().into()),
        })
    }
}

impl From<&str> for Input {
    fn from(s: &str) -> Self {
        Input::from_string(s)
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
    pub index: u32,
    pub line: u32,
    pub column: u32,
}
