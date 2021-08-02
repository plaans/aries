use anyhow::*;
use aries_utils::disp_iter;
use aries_utils::input::*;
use std::convert::TryInto;
use std::fmt::{Debug, Display, Formatter};
use std::intrinsics::unreachable;

pub type SAtom = aries_utils::input::Sym;

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

    pub fn loc(&self) -> Loc {
        Loc::new(&self.source, self.span)
    }

    pub fn invalid(&self, error: impl Into<String>) -> ErrLoc {
        self.loc().invalid(error)
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
    // pub fn source(&self) -> &std::sync::Arc<Input> {
    //     match self {
    //         SExpr::Atom(a) => &a.loc(),
    //         SExpr::List(l) => &l.source,
    //     }
    // }
    //
    // pub fn span(&self) -> Span {
    //     match self {
    //         SExpr::Atom(a) => a.span,
    //         SExpr::List(l) => l.span,
    //     }
    // }

    pub fn loc(&self) -> Loc {
        match self {
            SExpr::Atom(atom) => atom.loc(),
            SExpr::List(list) => list.loc(),
        }
    }

    pub fn invalid(&self, error: impl Into<String>) -> ErrLoc {
        self.loc().invalid(error)
    }

    pub fn is_atom(&self, expected_atom: &str) -> bool {
        self.as_atom().map(|a| a.as_str() == expected_atom).unwrap_or(false)
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
pub struct ListIter<'a> {
    elems: &'a [SExpr],
    source: std::sync::Arc<Input>,
    span: Span,
}

impl<'a> ListIter<'a> {
    pub fn peek(&self) -> Option<&'a SExpr> {
        self.elems.first()
    }

    pub fn pop(&mut self) -> std::result::Result<&'a SExpr, ErrLoc> {
        self.next()
            .ok_or_else(|| self.loc().end().invalid("Unexpected end of list"))
    }

    pub fn loc(&self) -> Loc {
        Loc::new(&self.source, self.span)
    }

    pub fn invalid(&self, error: impl Into<String>) -> ErrLoc {
        self.loc().invalid(error)
    }

    pub fn len(&self) -> usize {
        self.elems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elems.is_empty()
    }

    pub fn pop_known_atom(&mut self, expected: &str) -> std::result::Result<(), ErrLoc> {
        match self.next() {
            None => Err(self
                .loc()
                .end()
                .invalid(format!("Expected atom {} but got end of list", expected))),

            Some(sexpr) => {
                let sexpr = sexpr
                    .as_atom()
                    .ok_or_else(|| sexpr.invalid(format!("Expected atom `{}`", expected)))?;
                if sexpr.as_str() == expected {
                    Ok(())
                } else {
                    Err(sexpr.invalid(format!("Expected the atom `{}`", expected)))
                }
            }
        }
    }

    pub fn pop_atom(&mut self) -> std::result::Result<&SAtom, ErrLoc> {
        match self.next() {
            None => Err(self.loc().end().invalid("Expected an atom but got end of list.")),
            Some(sexpr) => sexpr.as_atom().ok_or_else(|| sexpr.invalid("Expected an atom")),
        }
    }
    pub fn pop_list(&mut self) -> std::result::Result<&SList, ErrLoc> {
        match self.next() {
            None => Err(self.loc().end().invalid("Expected a list but got end of list.")),
            Some(sexpr) => sexpr.as_list().ok_or_else(|| sexpr.invalid("Expected a list")),
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
            SExpr::Atom(a) => write!(f, "{}", a),
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
    Quote(Pos),
    QuasiQuote(Pos),
    Unquote(Pos),
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
        if n.is_whitespace() || n == '(' || n == ')' || n == ';' || is_in_comment
            //For quote, quasiquote and unquote support
            || n == '\'' || n == '`' || n == ','
        {
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
                } else if n == '\'' {
                    tokens.push(Token::Quote(pos));
                } else if n == '`' {
                    tokens.push(Token::QuasiQuote(pos));
                } else if n == ',' {
                    tokens.push(Token::Unquote(pos));
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
            let span = Span {
                start: *start_pos,
                end: Pos {
                    line: start_pos.line,
                    column: start_pos.column + (s.len() as u32) - 1,
                },
            };
            let loc = Loc::new(src, span);
            let atom = Sym::with_source(s, loc);

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
        Some(quoting) => {
            let (sym_quote, start) = match quoting {
                Token::Quote(s) => (Sym::new("quote"), s),
                Token::QuasiQuote(s) => (Sym::new("quasiquote"), s),
                Token::Unquote(s) => (Sym::new("unquote"), s),
                _ => unreachable!("Unexpected token, should be Quote, QuasiQuote or Unquote"),
            };

            let mut es = vec![SExpr::Atom(sym_quote)];
            let e = read(tokens, src)?;
            //let loc = e.loc();
            es.push(e);
            //Compute the span
            Ok(SExpr::List(SList {
                list: es,
                source: src.clone(),
                span: Span::new(*start, *start), //TODO: find a way to declare the span
            }))
        }
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
    fn parsing_quotting() {
        formats_as("'x", "(quote x)");
        formats_as("`x", "(quasiquote x)");
        formats_as(",x", "(unquote x)");
        formats_as("'(x)", "(quote (x))");
        formats_as("`(x)", "(quasiquote (x))");
        formats_as(",(x)", "(unquote (x))");
        formats_as("('x)", "((quote x))");
        formats_as("('(x))", "((quote (x)))");
        formats_as("('x 'y)", "((quote x) (quote y))")
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
        let result = format!("{}", sexpr.loc().underlined());
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
