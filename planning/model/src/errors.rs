use std::{
    fmt::{Debug, Display},
    ops::Range,
    sync::Arc,
};

use crate::{Environment, Sym, pddl::input::Input};
use annotate_snippets::*;
use thiserror::Error;

pub type SrcRange = Range<usize>;

/// A substring of a file, with metadata for displaying (filename, indices, ...)
#[derive(Clone)]
pub struct Span {
    input: Arc<Input>,
    span: SrcRange,
}
pub type OSpan = Option<Span>;

impl Span {
    pub fn new(input: Arc<Input>, first: usize, last: usize) -> Self {
        Span {
            input,
            span: first..(last + 1),
        }
    }

    pub fn str(&self) -> &str {
        &self.input.text.as_str()[self.span.clone()]
    }

    pub fn annotate(&self, lvl: Level<'static>, message: impl ToString) -> Annot {
        let message = message.to_string();
        // build a source from the object itself
        Annot {
            level: lvl,
            span: self.clone(),
            message,
        }
    }

    pub fn error(&self, message: impl ToString) -> Annot {
        self.annotate(Level::ERROR, message)
    }

    pub fn info(&self, message: impl ToString) -> Annot {
        self.annotate(Level::INFO, message)
    }

    pub fn end(self) -> Self {
        let last = self.span.last().unwrap();
        Self {
            input: self.input,
            span: last..(last + 1),
        }
    }
    pub fn invalid(&self, msg: impl ToString) -> Message {
        let msg = msg.to_string();
        if self.span.len() < 40 {
            Message::error(format!("{msg}: {}", self.str())).snippet(self.clone().error(msg))
        } else {
            Message::error(&msg).snippet(self.clone().error(msg))
        }
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

    fn loc(&self) -> Span {
        self.span_or_default()
    }

    fn invalid(&self, msg: impl ToString) -> Message {
        let msg = msg.to_string();
        let span = self.span_or_default();
        if span.span.len() < 40 {
            // symbol seems short enough write it inline in the message
            Message::error(format!("{msg}: {}", span.str())).snippet(span.error(msg))
        } else {
            Message::error(&msg).snippet(span.error(msg))
        }
    }

    fn error(&self, message: impl ToString) -> Annot {
        self.annotate(Level::ERROR, message)
    }

    fn info(&self, message: impl ToString) -> Annot {
        self.annotate(Level::INFO, message)
    }

    fn annotate(&self, lvl: Level<'static>, message: impl ToString) -> Annot {
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
    level: Level<'static>,
    span: Span,
    message: String,
}

impl Annot {
    fn build(&self) -> Snippet<'_, Annotation<'_>> {
        let annotation_kind = match self.level {
            Level::ERROR => AnnotationKind::Primary,
            _ => AnnotationKind::Context,
        };
        let annotation = annotation_kind.span(self.span.span.clone()).label(&self.message);
        let snippet = Snippet::source(&self.span.input.text)
            .line_start(1)
            .fold(true)
            .annotation(annotation);
        if let Some(file) = self.span.input.source.as_ref() {
            snippet.path(file.as_str())
        } else {
            snippet
        }
    }
}

impl Spanned for &Sym {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

#[derive(Error)]
pub struct Message {
    level: Level<'static>,
    title: String,
    snippets: Vec<Annot>,
    info: Vec<String>,
}

impl Message {
    pub fn new(level: Level<'static>, title: impl ToString) -> Self {
        Self {
            level,
            title: title.to_string(),
            snippets: Vec::new(),
            info: Vec::new(),
        }
    }

    pub fn error(title: impl ToString) -> Self {
        Self::new(Level::ERROR, title)
    }

    pub fn snippet(mut self, snippet: Annot) -> Self {
        self.snippets.push(snippet);
        self
    }

    pub fn info(self, s: impl Spanned, msg: &str) -> Message {
        let annot = s.annotate(Level::INFO, msg);
        self.snippet(annot)
    }

    pub fn ctx(mut self, s: impl ToString) -> Message {
        self.info.push(s.to_string());
        self
    }

    pub fn failed<T>(self) -> std::result::Result<T, Message> {
        Err(self)
    }
}

pub trait Ctx<T> {
    fn ctx(self, error_context: impl Display) -> std::result::Result<T, Message>;
}
impl<T> Ctx<T> for std::result::Result<T, Message> {
    fn ctx(self, error_context: impl Display) -> Result<T, Message> {
        self.map_err(|e| e.ctx(error_context))
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let renderer = Renderer::styled();
        let disp = self
            .level
            .clone()
            .primary_title(&self.title)
            .elements(self.snippets.iter().map(|s| s.build()));
        let disp = renderer.render(&[disp]);
        f.write_str(&disp)
    }
}
impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl<T> ErrorMessageExt<T> for Result<T, Message> {
    fn with_info(self, annot: impl FnOnce() -> Annot) -> Result<T, Message> {
        self.map_err(|m| m.snippet(annot()))
    }

    fn ctx2(self, tagged: impl Spanned, tag: impl ToString) -> Result<T, Message> {
        self.with_info(|| tagged.info(tag))
    }
}

pub trait ErrorMessageExt<T> {
    fn with_info(self, annot: impl FnOnce() -> Annot) -> Result<T, Message>;
    fn ctx2(self, tagged: impl Spanned, tag: impl ToString) -> Result<T, Message>;
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
