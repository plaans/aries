use std::{
    fmt::{Debug, Display},
    ops::Range,
    sync::Arc,
};

use crate::{
    Environment,
    pddl::input::{ErrLoc, Input},
};
use annotate_snippets::*;
use thiserror::Error;

#[derive(Clone)]
pub struct Span {
    input: Arc<Input>,
    span: Range<usize>,
}
pub type OSpan = Option<Span>;

impl Span {
    pub fn annotate(&self, lvl: Level, message: impl ToString) -> Annot {
        let message = message.to_string();
        // build a source from the object itself
        Annot {
            level: lvl,
            span: self.clone(),
            message,
        }
    }

    pub fn error(&self, message: impl ToString) -> Annot {
        self.annotate(Level::Error, message)
    }

    pub fn info(&self, message: impl ToString) -> Annot {
        self.annotate(Level::Info, message)
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[span]")
    }
}

pub trait Spanned: Display {
    fn span(&self) -> Option<&Span>;

    fn span_or_default(&self) -> Span {
        self.span().cloned().unwrap_or_else(|| {
            let text = self.to_string();
            Input::from_string(self);
            let span = 0..text.chars().count();
            Span {
                input: Arc::new(Input::from_string(text)),
                span,
            }
        })
    }

    fn error(&self, message: impl ToString) -> Annot {
        self.annotate(Level::Error, message)
    }

    fn info(&self, message: impl ToString) -> Annot {
        self.annotate(Level::Info, message)
    }

    fn annotate(&self, lvl: Level, message: impl ToString) -> Annot {
        let message = message.to_string();
        // build a source from the object itself
        Annot {
            level: lvl,
            span: self.span_or_default(),
            message,
        }
    }
}

pub struct Annot {
    level: Level,
    span: Span,
    message: String,
}

impl Annot {
    pub fn build(&self) -> Snippet<'_> {
        let annotation = self.level.span(self.span.span.clone()).label(&self.message);
        let snippet = Snippet::source(&self.span.input.text)
            .line_start(1)
            .fold(true)
            .annotation(annotation);
        if let Some(file) = self.span.input.source.as_ref() {
            snippet.origin(file.as_str())
        } else {
            snippet
        }
    }
}

impl Spanned for &crate::Sym {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

#[derive(Error)]
pub struct Message {
    level: Level,
    title: String,
    snippets: Vec<Annot>,
}

impl Message {
    pub fn new(level: Level, title: impl ToString) -> Self {
        Self {
            level,
            title: title.to_string(),
            snippets: Vec::new(),
        }
    }

    pub fn error(title: impl ToString) -> Self {
        Self::new(Level::Error, title)
    }

    pub fn snippet(mut self, snippet: Annot) -> Self {
        self.snippets.push(snippet);
        self
    }

    pub fn info(self, s: impl Spanned, msg: &str) -> Message {
        let annot = s.annotate(Level::Info, msg);
        self.snippet(annot)
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let renderer = Renderer::styled();
        let disp = self
            .level
            .title(&self.title)
            .snippets(self.snippets.iter().map(|s| s.build()));
        let disp = format!("{}", renderer.render(disp));
        f.write_str(&disp)
    }
}
impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl From<ErrLoc> for Message {
    fn from(value: ErrLoc) -> Self {
        let message_string = value.inline_err.unwrap_or("error".to_string());
        let message = Message::error(&message_string);
        if let Some(loc) = value.loc {
            let span = Span::from(loc);
            message.snippet(span.annotate(Level::Error, message_string))
        } else {
            message
        }
    }
}

impl<T> ErrorMessageExt<T> for Result<T, Message> {
    fn with_info(self, annot: impl FnOnce() -> Annot) -> Result<T, Message> {
        self.map_err(|m| m.snippet(annot()))
    }
}

pub trait ErrorMessageExt<T> {
    fn with_info(self, annot: impl FnOnce() -> Annot) -> Result<T, Message>;
}

// pub struct Span(Arc<dyn SourceLoc>);

impl From<crate::pddl::input::Loc> for Span {
    fn from(value: crate::pddl::input::Loc) -> Self {
        let (start, end) = value.source.indices(value.span()).unwrap();
        Span {
            input: value.source.clone(),
            span: start..(end + 1),
        }
    }
}

impl From<crate::pddl::input::Loc> for OSpan {
    fn from(value: crate::pddl::input::Loc) -> Self {
        Some(value.into())
    }
}

pub(crate) trait ToEnvMessage {
    fn to_message(self, env: &Environment) -> Message;
}

pub(crate) trait EnvError<T> {
    fn msg(self, env: &Environment) -> Result<T, Message>;
}

impl<T, E: ToEnvMessage> EnvError<T> for Result<T, E> {
    fn msg(self, env: &Environment) -> Result<T, Message> {
        self.map_err(|e| e.to_message(env))
    }
}
