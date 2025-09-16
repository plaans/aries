use std::convert::TryFrom;

pub struct Input {
    pub(crate) text: String,
    pub(crate) source: Option<String>,
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

    pub fn snippet(&self) -> annotate_snippets::Snippet<'_> {
        let snippet = annotate_snippets::Snippet::source(&self.text).line_start(1).fold(true);
        if let Some(file) = &self.source {
            snippet.origin(file)
        } else {
            snippet
        }
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
