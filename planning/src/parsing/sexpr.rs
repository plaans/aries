use anyhow::*;
use aries_utils::{disp_iter, Fmt};

use std::fmt::{Debug, Display, Formatter};
use std::ops::Index;

#[derive(Eq, PartialEq, Clone)]
pub struct SExpr<'a> {
    e: Expr<'a>,
    src: &'a str,
    start: usize,
    end: usize,
}

impl<'a> std::ops::Index<usize> for SExpr<'a> {
    type Output = SExpr<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        match &self.e {
            Expr::List(v) => &v[index],
            _ => panic!("Tried to use an index on an atom"),
        }
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum Expr<'a> {
    Atom,
    List(Vec<SExpr<'a>>),
}

impl<'a> SExpr<'a> {
    pub fn atom(src: &'a str, start: usize, end: usize) -> Self {
        SExpr {
            e: Expr::Atom,
            src,
            start,
            end,
        }
    }

    pub fn new(es: Vec<SExpr<'a>>, src: &'a str, start: usize, end: usize) -> Self {
        SExpr {
            e: Expr::List(es),
            src,
            start,
            end,
        }
    }

    pub fn display_with_context(&self) -> impl std::fmt::Display + '_ {
        let formatter = move |f: &mut std::fmt::Formatter| {
            let mut line_start = 0;
            for l in self.src.lines() {
                let line_end = line_start + l.len();

                if line_start <= self.start && self.start < line_end {
                    // writeln!(f, "=={}==", &self.src[self.start..=self.end])?;
                    writeln!(f, "{}", l)?;
                    let index = self.start - line_start;
                    let length = self.end.min(line_end - 1) - self.start + 1;
                    writeln!(f, "{}{}", " ".repeat(index), "^".repeat(length))?;
                }

                line_start = line_end + 1;
            }
            Ok(())
        };
        Fmt(formatter)
    }
    //
    // #[allow(dead_code)]
    // pub fn map<G, F: Fn(&Str) -> G + Copy>(&self, f: F) -> Expr<G> {
    //     match self {
    //         Expr::Atom(a) => Expr::Atom(f(a)),
    //         Expr::List(v) => Expr::List(v.iter().map(|e| e.map(f)).collect()),
    //     }
    // }

    pub fn into_list(self) -> Option<Vec<SExpr<'a>>> {
        match self.e {
            Expr::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&'a [SExpr]> {
        match &self.e {
            Expr::List(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    pub fn as_list_iter(&self) -> Option<ListIter> {
        match &self.e {
            Expr::List(v) => Some(ListIter { elems: v.as_slice() }),
            _ => None,
        }
    }

    pub fn into_atom(self) -> Option<&'a str> {
        self.as_atom()
    }

    pub fn as_atom(&self) -> Option<&'a str> {
        match self.e {
            Expr::Atom => Some(&self.src[self.start..=self.end]),
            _ => None,
        }
    }

    // pub fn as_application_args<X: ?Sized>(&self, f: &X) -> Option<&[Expr<Str>]>
    // where
    //     Str: Borrow<X>,
    //     X: Eq + PartialEq,
    // {
    //     match &self.e {
    //         Expr::List(v) => match &v.first() {
    //             Some(Expr::Atom(head)) if head.borrow() == f => Some(&v[1..]),
    //             _ => None,
    //         },
    //         _ => None,
    //     }
    // }
}

pub struct ListIter<'a> {
    elems: &'a [SExpr<'a>],
}

impl<'a> ListIter<'a> {
    pub fn next(&mut self) -> Option<&SExpr<'a>> {
        match self.elems.split_first() {
            None => None,
            Some((head, tail)) => {
                self.elems = tail;
                Some(head)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.elems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elems.is_empty()
    }

    pub fn pop_known_atom(&mut self, expected: &str) -> Result<()> {
        match self.next() {
            None => bail!("oups"),
            Some(sexpr) => match sexpr.as_atom() {
                Some(x) if x == expected => Ok(()),
                _ => {
                    println!(
                        "Expected atom \"{}\" but got:\n{}",
                        expected,
                        sexpr.display_with_context()
                    );

                    bail!("oups")
                }
            },
        }
    }

    pub fn pop_atom(&mut self) -> Result<&str> {
        match self.next() {
            None => bail!("oups"),
            Some(sexpr) => sexpr.as_atom().context("expected an atom"),
        }
    }
    pub fn pop_list(&mut self) -> Result<ListIter> {
        match self.next() {
            None => bail!("oups"),
            Some(sexpr) => sexpr.as_list_iter().context("expected a list"),
        }
    }
}

impl<'a> Display for SExpr<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.e {
            Expr::Atom => write!(f, "{}", self.as_atom().unwrap()),
            Expr::List(v) => {
                write!(f, "(")?;
                disp_iter(f, v.as_slice(), " ")?;
                write!(f, ")")
            }
        }
    }
}

impl<'a> Debug for SExpr<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Sym(usize, usize),
    LParen(usize),
    RParen(usize),
}

pub fn parse(s: &str) -> Result<SExpr> {
    let tokenized = tokenize(&s);
    let mut tokens = tokenized.iter().peekable();
    read(&mut tokens, s)
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars = &mut s.chars();

    let mut cur_start = None;
    let mut index = 0;
    while let Some(n) = chars.next() {
        if n == ';' {
            // drop all chars until a new line is found, counting to force consuming the iterator.
            index += chars.take_while(|c| *c != '\n').count();
        } else if n.is_whitespace() || n == '(' || n == ')' {
            if let Some(start) = cur_start {
                tokens.push(Token::Sym(start, index - 1));
                cur_start = None;
            }
            if n == '(' {
                tokens.push(Token::LParen(index));
            }
            if n == ')' {
                tokens.push(Token::RParen(index));
            }
        } else if cur_start == None {
            cur_start = Some(index);
        }
        index += 1;
    }
    if let Some(start) = cur_start {
        tokens.push(Token::Sym(start, index - 1));
    }
    tokens
}

fn read<'a>(tokens: &mut std::iter::Peekable<core::slice::Iter<Token>>, src: &'a str) -> Result<SExpr<'a>> {
    match tokens.next() {
        Some(Token::Sym(start, end)) => {
            let expr = SExpr::atom(src, *start, *end);
            Ok(expr)
        }
        Some(Token::LParen(start)) => {
            let mut es = Vec::new();
            loop {
                match tokens.peek() {
                    Some(Token::RParen(end)) => {
                        let _ = tokens.next(); // consume
                        break Ok(SExpr::new(es, src, *start, *end));
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
        assert_eq!(parse("aa").unwrap().as_atom(), Some("aa"));
        formats_as("aa", "aa");
        formats_as(" aa", "aa");
        formats_as("aa ", "aa");
        formats_as(" aa ", "aa");
        formats_as("(a b)", "(a b)");
        formats_as("(a b)", "(a b)");
        formats_as("(a (b c) d)", "(a (b c) d)");
        formats_as(" ( a  ( b  c )   d  )   ", "(a (b c) d)");
    }

    fn displayed_as(sexpr: &SExpr, a: &str, b: &str) {
        let result = format!("{}", sexpr.display_with_context());
        let expected = format!("{}\n{}\n", a, b);
        println!("=============\nResult:\n{}Expected:\n{}=============", result, expected);
        assert_eq!(&result, &expected);
    }

    #[test]
    #[rustfmt::skip]
    fn contextual_display() {
        let ex = parse("( a (b c))").unwrap();
        displayed_as(&ex,
                     "( a (b c))",
                     "^^^^^^^^^^");
        displayed_as(&ex[0],
                     "( a (b c))",
                     "  ^");
        displayed_as(&ex[1],
                     "( a (b c))",
                     "    ^^^^^");
        displayed_as(&ex[1][0],
                     "( a (b c))",
                     "     ^");
        displayed_as(&ex[1][1], 
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
