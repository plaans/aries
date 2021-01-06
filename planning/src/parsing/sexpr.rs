use anyhow::*;
use aries_utils::disp_iter;
use aries_utils::input::{Input, Pos, Span};
use std::convert::TryInto;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

#[derive(Clone)]
pub struct SAtom {
    /// Name of the atom, in lower case
    normalized_name: String,
    pub source: std::sync::Arc<Input>,
    pub position: Pos, // TODO: use span, since normalization might change the number of chars
}

impl SAtom {
    pub fn as_str(&self) -> &str {
        self.normalized_name.as_str()
    }

    pub fn span(&self) -> Span {
        let start = self.position;
        let end = Pos {
            line: start.line,
            column: start.column + self.normalized_name.len() as u32 - 1,
        };
        Span { start, end }
    }
}

impl std::fmt::Display for SAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.normalized_name)
    }
}

#[derive(Clone)]
pub struct SList {
    list: Vec<SExpr>,
    source: std::sync::Arc<Input>,
    span: Span,
}

impl SList {
    pub fn iter(&self) -> ListIter {
        ListIter {
            elems: self.list.as_slice(),
            source: self.source.clone(),
            span: self.span,
        }
    }
}

impl std::ops::Index<usize> for SList {
    type Output = SExpr;

    fn index(&self, index: usize) -> &Self::Output {
        &self.list[index]
    }
}

#[derive(Clone)]
pub enum SExpr {
    Atom(SAtom),
    List(SList),
}

impl SExpr {
    pub fn source(&self) -> &std::sync::Arc<Input> {
        match self {
            SExpr::Atom(a) => &a.source,
            SExpr::List(l) => &l.source,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            SExpr::Atom(a) => a.span(),
            SExpr::List(l) => l.span,
        }
    }

    /// If this s-expression is the application of the function `function_name`, returns
    /// the arguments of the application.
    ///
    /// ```
    /// use aries_planning::parsing::sexpr::parse;
    /// let sexpr = parse("(add 1 2)").unwrap();
    /// let args = sexpr.as_application("add").unwrap(); // returns the list equivalent of [1, 2]
    /// assert_eq!(args[0].as_atom().unwrap().as_str(), "1");
    /// assert_eq!(args[1].as_atom().unwrap().as_str(), "2");
    /// ```
    pub fn as_application(&self, function_name: &str) -> Option<&[SExpr]> {
        match self {
            SExpr::Atom(_) => None,
            SExpr::List(l) => match l.list.as_slice() {
                [SExpr::Atom(head), rest @ ..] if head.as_str() == function_name => Some(rest),
                _ => None,
            },
        }
    }
}
impl SExpr {
    pub fn as_list(&self) -> Option<&SList> {
        match &self {
            SExpr::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_list_iter(&self) -> Option<ListIter> {
        match &self {
            SExpr::List(v) => Some(ListIter {
                elems: v.list.as_slice(),
                source: v.source.clone(),
                span: v.span,
            }),
            _ => None,
        }
    }

    pub fn as_atom(&self) -> Option<&SAtom> {
        match self {
            SExpr::Atom(a) => Some(a),
            _ => None,
        }
    }
}

pub struct ErrLoc {
    context: Vec<String>,
    inline_err: Option<String>,
    loc: Option<(std::sync::Arc<Input>, Span)>,
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
        if let Some((source, span)) = &self.loc {
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

pub trait Localized<T> {
    fn localized(self, source: &std::sync::Arc<Input>, span: Span) -> std::result::Result<T, ErrLoc>;
}
impl<T> Localized<T> for Option<T> {
    fn localized(self, source: &Arc<Input>, span: Span) -> Result<T, ErrLoc> {
        match self {
            Some(x) => Ok(x),
            None => Err(ErrLoc {
                context: Vec::new(),
                inline_err: None,
                loc: Some((source.clone(), span)),
            }),
        }
    }
}
impl<T, E: Display> Localized<T> for Result<T, E> {
    fn localized(self, source: &Arc<Input>, span: Span) -> Result<T, ErrLoc> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ErrLoc {
                context: Vec::new(),
                inline_err: Some(format!("{}", e)),
                loc: Some((source.clone(), span)),
            }),
        }
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

pub struct ListIter<'a> {
    elems: &'a [SExpr],
    source: std::sync::Arc<Input>,
    span: Span,
}

impl<'a> ListIter<'a> {
    pub fn pop(&mut self) -> std::result::Result<&'a SExpr, ErrLoc> {
        self.next()
            .ok_or("Unexpected end of list")
            .localized(&self.source, Span::point(self.span.end))
    }

    pub fn len(&self) -> usize {
        self.elems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elems.is_empty()
    }

    pub fn pop_known_atom(&mut self, expected: &str) -> std::result::Result<(), ErrLoc> {
        match self.next() {
            None => Err(format!("Expected atom {} but got end of list", expected))
                .localized(&self.source, Span::point(self.span.end)),
            Some(sexpr) => {
                let sexpr = sexpr
                    .as_atom()
                    .ok_or(format!("Expected atom `{}`", expected))
                    .localized(sexpr.source(), sexpr.span())?;
                if sexpr.as_str() == expected {
                    Ok(())
                } else {
                    Err(format!("Expected the atom `{}`", expected)).localized(&sexpr.source, sexpr.span())
                }
            }
        }
    }

    pub fn pop_atom(&mut self) -> std::result::Result<&SAtom, ErrLoc> {
        match self.next() {
            None => Err("Expected an atom but got end of list.").localized(&self.source, Span::point(self.span.end)),
            Some(sexpr) => sexpr
                .as_atom()
                .ok_or("Expected an atom")
                .localized(sexpr.source(), sexpr.span()),
        }
    }
    pub fn pop_list(&mut self) -> std::result::Result<&SList, ErrLoc> {
        match self.next() {
            None => Err("Expected a list but got end of list.").localized(&self.source, Span::point(self.span.end)),
            Some(sexpr) => sexpr
                .as_list()
                .ok_or("Expected a list")
                .localized(sexpr.source(), sexpr.span()),
        }
    }
}

impl<'a> Iterator for ListIter<'a> {
    type Item = &'a SExpr;

    fn next(&mut self) -> Option<Self::Item> {
        match self.elems.split_first() {
            None => None,
            Some((head, tail)) => {
                self.elems = tail;
                Some(head)
            }
        }
    }
}

impl Display for SExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            SExpr::Atom(a) => write!(f, "{}", a.normalized_name),
            SExpr::List(l) => {
                write!(f, "(")?;
                disp_iter(f, &l.list, " ")?;
                write!(f, ")")
            }
        }
    }
}

impl Debug for SExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Sym { start: usize, end: usize, start_pos: Pos },
    LParen(Pos),
    RParen(Pos),
}

pub fn parse<S: TryInto<Input>>(s: S) -> Result<SExpr>
where
    <S as TryInto<Input>>::Error: std::error::Error + Send + Sync + 'static,
{
    let s = s.try_into()?;
    let s = std::sync::Arc::new(s);
    let tokenized = tokenize(s.clone());
    let mut tokens = tokenized.iter().peekable();
    read(&mut tokens, &s)
}

/// Parse the input into a sequence of tokens.
fn tokenize(source: std::sync::Arc<Input>) -> Vec<Token> {
    let s = source.text.as_str();
    let mut tokens = Vec::new();

    // current index into `s`
    let mut index = 0;
    // start index of the current atom
    let mut cur_start = None;

    // current line number (starts a 0)
    let mut line: usize = 0;
    // index of the start of the line
    let mut line_start = 0;

    // true if we are currently inside a comment (between a ';' and a '\n')
    let mut is_in_comment = false;

    // creates a new symbol token
    let make_sym = |start, end, line, line_start| {
        let start_pos = Pos {
            line: line as u32,
            column: (start - line_start) as u32,
        };
        Token::Sym { start, end, start_pos }
    };

    for n in s.chars() {
        if n.is_whitespace() || n == '(' || n == ')' || n == ';' || is_in_comment {
            // if we were parsing a symbol, we have reached its end
            if let Some(start) = cur_start {
                tokens.push(make_sym(start, index - 1, line, line_start));
                cur_start = None;
            }

            if n == '\n' {
                // switch to next line and exit comment mode
                line += 1;
                line_start = index + 1; // line will start at the next character
                is_in_comment = false;
            } else if n == ';' {
                is_in_comment = true;
            } else if !is_in_comment {
                let pos = Pos {
                    line: line as u32,
                    column: (index - line_start) as u32,
                };
                if n == '(' {
                    tokens.push(Token::LParen(pos));
                } else if n == ')' {
                    tokens.push(Token::RParen(pos));
                }
            }
        } else if cur_start == None {
            cur_start = Some(index);
        }
        index += 1;
    }
    if let Some(start) = cur_start {
        tokens.push(make_sym(start, index - 1, line, line_start));
    }
    tokens
}

fn read(tokens: &mut std::iter::Peekable<core::slice::Iter<Token>>, src: &std::sync::Arc<Input>) -> Result<SExpr> {
    match tokens.next() {
        Some(Token::Sym { start, end, start_pos }) => {
            let s = &src.text.as_str()[*start..=*end];
            let s = s.to_ascii_lowercase();
            let atom = SAtom {
                normalized_name: s,
                source: src.clone(),
                position: *start_pos,
            };

            Ok(SExpr::Atom(atom))
        }
        Some(Token::LParen(start)) => {
            let mut es = Vec::new();
            loop {
                match tokens.peek() {
                    Some(Token::RParen(end)) => {
                        let _ = tokens.next(); // consume
                        let list = SList {
                            list: es,
                            source: src.clone(),
                            span: Span::new(*start, *end),
                        };
                        break Ok(SExpr::List(list));
                    }
                    _ => {
                        let e = read(tokens, src)?;
                        es.push(e);
                    }
                }
            }
        }
        Some(Token::RParen(_)) => bail!("Unexpected closing parenthesis"),
        None => bail!("Unexpected end of output"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formats_as(input: &str, output: &str) {
        let res = parse(input).unwrap();
        let formatted = format!("{}", res);
        assert_eq!(&formatted, output);
    }

    #[test]
    fn parsing() {
        formats_as("aa", "aa");
        formats_as("aa", "aa");
        formats_as(" aa", "aa");
        formats_as("aa ", "aa");
        formats_as(" aa ", "aa");
        formats_as("(a b)", "(a b)");
        formats_as("(a b)", "(a b)");
        formats_as("(a (b c) d)", "(a (b c) d)");
        formats_as(" ( a  ( b  c )   d  )   ", "(a (b c) d)");
        formats_as(
            " ( a  (  
        b  c )   d  )   ",
            "(a (b c) d)",
        );
        formats_as(
            " ( a  ( b ; (y x)
         c )   d
           )  
          ",
            "(a (b c) d)",
        );
    }

    fn displayed_as(sexpr: &SExpr, a: &str, b: &str) {
        let result = format!("{}", sexpr.source().underlined(sexpr.span()));
        let expected = format!("{}\n{}", a, b);
        println!(
            "=============\nResult:\n{}\nExpected:\n{}\n=============",
            result, expected
        );
        assert_eq!(&result, &expected);
    }

    #[test]
    #[rustfmt::skip]
    fn contextual_display() {
        let ex = parse("( a (b c))").unwrap();
        displayed_as(&ex,
                     "( a (b c))",
                     "^^^^^^^^^^");
        let ex = ex.as_list().unwrap();
        displayed_as(&ex[0],
                     "( a (b c))",
                     "  ^");
        displayed_as(&ex[1],
                     "( a (b c))",
                     "    ^^^^^");
        displayed_as(&ex[1].as_list().unwrap()[0],
                     "( a (b c))",
                     "     ^");
        displayed_as(&ex[1].as_list().unwrap()[1], 
                     "( a (b c))",
                     "       ^");
        
        let src = " \n
(a (b c 
    d (e f g))\n
)";
        let src = parse(src).unwrap();
        displayed_as(
            &src, 
            "(a (b c ",
            "^^^^^^^^"
        );
    }
}
