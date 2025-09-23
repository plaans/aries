use std::{convert::TryFrom, hash::Hash};

/// Represents an input string, typically from a file but designed to also accomate in memory strings.
#[derive(Eq)]
pub struct Input {
    /// Text of the input
    pub(crate) text: String,
    /// Identifier of the source (typically the path to the file)
    /// This one is use to indicate the source in error outputs.
    pub(crate) source: Option<String>,
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        match (&self.source, &other.source) {
            (Some(x), Some(y)) => x == y,
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
    pub fn from_string(input: impl ToString) -> Input {
        Input {
            text: input.to_string(),
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
    pub index: u32,
    pub line: u32,
    pub column: u32,
}
