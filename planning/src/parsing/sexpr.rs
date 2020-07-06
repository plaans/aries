use crate::utils::disp_iter;
use anyhow::*;
use std::borrow::Borrow;
use std::fmt::{Debug, Display, Error, Formatter};

#[derive(Eq, PartialEq, Clone)]
pub enum Expr<Atom> {
    Leaf(Atom),
    SExpr(Vec<Expr<Atom>>),
}

impl<E: Clone> Expr<E> {
    pub fn atom(e: E) -> Self {
        Expr::Leaf(e)
    }

    pub fn new(es: Vec<Expr<E>>) -> Self {
        Expr::SExpr(es)
    }

    #[allow(dead_code)]
    pub fn map<G, F: Fn(&E) -> G + Copy>(&self, f: F) -> Expr<G> {
        match self {
            Expr::Leaf(a) => Expr::Leaf(f(a)),
            Expr::SExpr(v) => Expr::SExpr(v.iter().map(|e| e.map(f)).collect()),
        }
    }

    pub fn into_sexpr(self) -> Option<Vec<Expr<E>>> {
        match self {
            Expr::SExpr(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_sexpr(&self) -> Option<&[Expr<E>]> {
        match self {
            Expr::SExpr(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    pub fn into_atom(self) -> Option<E> {
        match self {
            Expr::Leaf(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_atom(&self) -> Option<&E> {
        match self {
            Expr::Leaf(a) => Some(&a),
            _ => None,
        }
    }

    pub fn as_application_args<X: ?Sized>(&self, f: &X) -> Option<&[Expr<E>]>
    where
        E: Borrow<X>,
        X: Eq + PartialEq,
    {
        match self {
            Expr::SExpr(v) => match &v.first() {
                Some(Expr::Leaf(ref head)) if head.borrow() == f => Some(&v[1..]),
                _ => None,
            },
            _ => None,
        }
    }
}

impl<Atom: Display> Display for Expr<Atom> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            Expr::Leaf(a) => write!(f, "{}", a),
            Expr::SExpr(v) => {
                write!(f, "(")?;
                disp_iter(f, v.as_slice(), " ")?;
                write!(f, ")")
            }
        }
    }
}
impl<Atom: Display> Debug for Expr<Atom> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            Expr::Leaf(a) => write!(f, "{}", a),
            Expr::SExpr(v) => {
                write!(f, "(")?;
                disp_iter(f, v.as_slice(), " ")?;
                write!(f, ")")
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Sym(String),
    LParen,
    RParen,
}

pub fn parse(s: &str) -> Result<Expr<String>> {
    let tokenized = tokenize(&s);
    let mut tokens = tokenized.iter().peekable();
    read(&mut tokens)
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars = &mut s.chars();
    let mut cur = String::new();
    while let Some(n) = chars.next() {
        if n == ';' {
            // drop all chars until a new line is found, counting to force consuming the iterator.
            chars.take_while(|c| *c != '\n').count();
        } else if n.is_whitespace() || n == '(' || n == ')' {
            if !cur.is_empty() {
                // change to lower case (pddl language is case insensitive)
                cur.make_ascii_lowercase();
                tokens.push(Token::Sym(cur));
                cur = String::new();
            }
            if n == '(' {
                tokens.push(Token::LParen);
            }
            if n == ')' {
                tokens.push(Token::RParen);
            }
        } else {
            cur.push(n);
        }
    }
    tokens
}

fn read(tokens: &mut std::iter::Peekable<core::slice::Iter<Token>>) -> Result<Expr<String>> {
    match tokens.next() {
        Some(Token::Sym(s)) => Result::Ok(Expr::atom(s.to_string())),
        Some(Token::LParen) => {
            let mut es = Vec::new();
            while tokens.peek() != Some(&&Token::RParen) {
                let e = read(tokens)?;
                es.push(e);
            }
            let droped = tokens.next();
            assert!(droped == Some(&Token::RParen));
            Result::Ok(Expr::new(es))
        }
        Some(Token::RParen) => bail!("Unexpected closing parenthesis"),
        None => bail!("Unexpected end of output"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{Display, Error, Formatter};

    enum Partial<PlaceHolder, Final> {
        Pending(PlaceHolder),
        Complete(Final),
    }

    impl<PH: Display, F: Display> Display for Partial<PH, F> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            match self {
                Partial::Pending(x) => write!(f, "?{}", x),
                Partial::Complete(x) => write!(f, "{}", x),
            }
        }
    }

    #[test]
    fn test1() {
        let add = Expr::atom("ADD".to_string());
        let a = Expr::atom("A".to_string());
        let b = Expr::atom("_B".to_string());
        let e = Expr::new(vec![a, b]);
        let e2 = Expr::new(vec![add, e]);

        let partial = e2.map(|s| {
            if s.starts_with("_B") {
                Partial::Pending(s.clone())
            } else {
                Partial::Complete(s.clone())
            }
        });

        println!("{}", e2);
        println!("partial: {}", partial);
    }
}
