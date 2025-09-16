use crate::errors::{Message, Span, Spanned};
use std::convert::TryFrom;
use std::fmt::Display;

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

    // pub fn underlined_position(&self, pos: Pos) -> impl std::fmt::Display + '_ {
    //     self.underlined(Span { start: pos, end: pos })
    // }

    // pub fn underlined(&self, span: Span) -> impl std::fmt::Display + '_ {
    //     let formatter = move |f: &mut std::fmt::Formatter| {
    //         let l = self
    //             .text
    //             .lines()
    //             .dropping(span.start.line as usize)
    //             .next()
    //             .expect("Invalid span for this source");
    //         assert!((span.start.column as usize) < l.len());
    //         writeln!(f, "{l}")?;

    //         let num_spaces = span.start.column as usize;
    //         let length = if span.start.line != span.end.line {
    //             l.len() - (span.start.column as usize)
    //         } else {
    //             (span.end.column - span.start.column + 1) as usize
    //         };
    //         // print spaces in front of underline, attempting to have the same spacing by place tabulation
    //         // when their are some in the input.
    //         for c in l[0..num_spaces].chars() {
    //             let output = if c == '\t' { '\t' } else { ' ' };
    //             write!(f, "{output}")?;
    //         }

    //         write!(f, "{}", "^".repeat(length))?;

    //         Ok(())
    //     };
    //     Fmt(formatter)
    // }

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

// /// A slice of an input.
// /// Mostly used to produce localized error messages through the `invalid` method.
// #[derive(Clone)]
// pub struct Loc {
//     pub(crate) source: std::sync::Arc<Input>,
//     pub(crate) span: Span,
// }
pub type Loc = crate::Span;

impl Loc {
    // pub fn underlined(&self) -> impl Display + '_ {
    //     self.source.underlined(self.span)
    // }
}

#[derive(Debug)]
pub struct ErrLoc {
    pub(crate) context: Vec<String>,
    pub(crate) inline_err: Option<String>,
    pub(crate) loc: Option<Span>,
}

impl ErrLoc {
    pub fn with_error(mut self, inline_message: impl Into<String>) -> ErrLoc {
        self.inline_err = Some(inline_message.into());
        self
    }

    pub fn failed<T>(self) -> std::result::Result<T, ErrLoc> {
        Err(self)
    }

    pub fn msg(self) -> Message {
        self.into()
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

// impl std::fmt::Display for ErrLoc {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         for (i, context) in self.context.iter().rev().enumerate() {
//             let prefix = if i > 0 { "Caused by" } else { "Error" };
//             writeln!(f, "{prefix}: {context}")?;
//         }
//         let inline_err = self.inline_err.as_deref().unwrap_or("");
//         if let Some(Loc { source, span }) = &self.loc {
//             let snippet = source.snippet();
//             let (start, end) = source.indices(*span).unwrap();
//             let annotation = annotate_snippets::Level::Error.span(start..(end + 1)).label(inline_err);
//             let snippet = snippet.annotation(annotation);
//             let message = annotate_snippets::Level::Error.title("").snippet(snippet);
//             let rendered = annotate_snippets::Renderer::styled();
//             writeln!(f, "{}", rendered.render(message))?;
//             // if let Some(path) = &source.source {
//             //     writeln!(f, "{}:{}:{}", path, span.start.line + 1, span.start.column)?;
//             // }
//             // write!(f, "{}", source.underlined(*span))?;
//         } else if let Some(err) = &self.inline_err {
//             write!(f, " {err}")?;
//         }
//         Ok(())
//     }
// }

// impl std::fmt::Debug for ErrLoc {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{self}")
//     }
// }

pub trait Ctx<T> {
    fn ctx(self, error_context: impl Display) -> std::result::Result<T, ErrLoc>;
    fn msg(self) -> Result<T, Message>;
}
impl<T> Ctx<T> for std::result::Result<T, ErrLoc> {
    fn ctx(self, error_context: impl Display) -> Result<T, ErrLoc> {
        self.map_err(|mut e| {
            e.context.push(error_context.to_string());
            e
        })
    }

    fn msg(self) -> Result<T, Message> {
        self.map_err(|e| e.msg())
    }
}

pub type Sym = crate::Sym;

impl Sym {
    pub fn with_source(s: impl Into<String>, source: Loc) -> Sym {
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

    pub fn invalid(&self, error: impl Into<String>) -> ErrLoc {
        self.span_or_default().invalid(error)
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
