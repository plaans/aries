use std::{
    fmt::{Debug, Display},
    ops::Range,
};

pub type Res<T> = Result<T, Message>;

use crate::{Environment, Sym, pddl::input::Input};
use annotate_snippets::*;
use itertools::Itertools;

pub type SrcRange = Range<usize>;

/// A substring of a file, with metadata for displaying (filename, indices, ...)
#[derive(Clone)]
pub struct Span {
    input: Input,
    span: SrcRange,
}
pub type OSpan = Option<Span>;

impl Span {
    pub fn new(input: Input, first: usize, last: usize) -> Self {
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
        // we have no explicit title, derive it from the message, including `self` in it if it is short
        let title = if self.span.len() < 60 {
            format!("{msg}: {}", self.str())
        } else {
            msg.to_string()
        };
        Message::error(title).snippet(self.clone().error(msg))
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
            let span = 0..text.chars().count();
            Span {
                input: Input::from_string(text),
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
    fn build_annot(&self) -> Annotation<'_> {
        let annotation_kind = match self.level {
            Level::ERROR => AnnotationKind::Primary,
            _ => AnnotationKind::Context,
        };
        annotation_kind.span(self.span.span.clone()).label(&self.message)
    }
}

impl Spanned for &Sym {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

pub struct Message {
    level: Level<'static>,
    title: String,
    snippets: Vec<Annot>,
    /// Subsets of the input that should be displayed (without any annotation)
    visible: Vec<Span>,
}

impl Message {
    #[cold]
    pub fn new(level: Level<'static>, title: impl ToString) -> Self {
        Self {
            level,
            title: title.to_string(),
            snippets: Vec::new(),
            visible: Vec::new(),
        }
    }

    #[cold]
    pub fn error(title: impl ToString) -> Self {
        Self::new(Level::ERROR, title)
    }
    #[cold]
    pub fn warning(title: impl ToString) -> Self {
        Self::new(Level::WARNING, title)
    }
    pub fn to_warning(mut self) -> Self {
        self.level = Level::WARNING;
        self
    }

    #[cold]
    pub fn snippet(mut self, snippet: Annot) -> Self {
        self.snippets.push(snippet);
        self
    }

    /// Marks a given span as visible.
    /// This is mostly interpreted as a hint and will only be considered if there are annoations on the same source file.
    pub fn show(mut self, span: &Span) -> Self {
        self.visible.push(span.clone());
        self
    }

    #[cold]
    pub fn info(self, s: impl Spanned, msg: &str) -> Message {
        let annot = s.annotate(Level::INFO, msg);
        self.snippet(annot)
    }

    #[cold]
    pub fn title(mut self, s: impl ToString) -> Message {
        self.title = s.to_string();
        self
    }

    #[cold]
    pub fn failed<T>(self) -> std::result::Result<T, Message> {
        Err(self)
    }
}

pub trait Ctx<T> {
    fn title(self, error_context: impl Display) -> std::result::Result<T, Message>;
}
impl<T> Ctx<T> for std::result::Result<T, Message> {
    fn title(self, error_context: impl Display) -> Result<T, Message> {
        self.map_err(|e| e.title(error_context))
    }
}
impl<T> Ctx<T> for Option<T> {
    fn title(self, msg: impl Display) -> Result<T, Message> {
        self.ok_or_else(|| Message::error(msg))
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // create a snippet for each source file with all annotation
        let snippets_by_source = self.snippets.iter().into_group_map_by(|s| &s.span.input);
        let snippets = snippets_by_source
            .iter()
            .map(|(source, annots)| {
                let snippet = Snippet::source(&source.text).line_start(1).fold(true);
                let snippet = if let Some(file) = source.source.as_ref() {
                    snippet.path(file.as_str())
                } else {
                    snippet
                };
                let snippet = snippet.annotations(annots.iter().map(|a| a.build_annot()));
                // for each visiblity requirement in this source, add it to the snippet
                // note that if a visible span is not linked to ay annotated source, it will not be picked up (which is the desired be)
                let visibles_in_source = self
                    .visible
                    .iter()
                    .filter_map(|s| (&s.input == *source).then_some(s.span.clone()));
                snippet.annotations(visibles_in_source.map(|range| AnnotationKind::Visible.span(range)))
            })
            .collect_vec();

        let renderer = Renderer::styled();
        let disp = self.level.clone().primary_title(&self.title).elements(snippets);
        let disp = renderer.render(&[disp]);
        f.write_str(&disp)
    }
}
impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl<E> From<E> for Message
where
    E: core::error::Error,
{
    #[cold]
    fn from(error: E) -> Self {
        Message::error(error)
    }
}

impl<T> ErrorMessageExt<T> for Result<T, Message> {
    fn with_info(self, annot: impl FnOnce() -> Annot) -> Result<T, Message> {
        self.map_err(|m| m.snippet(annot()))
    }

    fn tag(self, tagged: impl Spanned, tag: impl ToString, visible: Option<&Span>) -> Result<T, Message> {
        let res = self.with_info(|| tagged.info(tag));
        if let Some(visible) = visible {
            res.map_err(|m| m.show(visible))
        } else {
            res
        }
    }
}

pub trait ErrorMessageExt<T> {
    fn with_info(self, annot: impl FnOnce() -> Annot) -> Result<T, Message>;
    fn tag(self, tagged: impl Spanned, tag: impl ToString, visible: Option<&Span>) -> Result<T, Message>;
}

pub(crate) trait ToEnvMessage {
    fn to_message(self, env: &Environment) -> Message;
}

impl<E: ToEnvMessage> ToEnvMessage for Box<E> {
    fn to_message(self, env: &Environment) -> Message {
        (*self).to_message(env)
    }
}

pub(crate) trait EnvError<T> {
    fn msg(self, env: &Environment) -> Result<T, Message>;
}

impl<T, E: ToEnvMessage> EnvError<T> for Result<T, E> {
    fn msg(self, env: &Environment) -> Result<T, Message> {
        self.map_err(|e| e.to_message(env))
    }
}
