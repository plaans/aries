use std::fmt::{Display, Formatter, Error, Debug};
use crate::chronicles::sexpr::Expr;

pub mod ddl;
pub mod typesystem;
pub mod sexpr;
pub mod strips;
pub mod enumerate;
pub mod state;
pub mod ref_store;
pub mod heuristics;
pub mod search;

pub struct Sym(String);

pub enum Op {
    Eq,
    Not,
    And,
    Or
}

pub enum AST<Op,N,Sym> {
    Atom(Sym),
    Unary(Op, N),
    Binary(Op, N, N),
    NAry(Op, Vec<N>)
}


trait Type where Self : Sized {
    fn name(&self) -> &str;
    fn params(&self) -> &[Self];
    fn return_type(&self) -> Self;
}

pub fn disp_iter<T: Display>(f: &mut Formatter<'_>, iterable: &[T], sep: &str) -> Result<(),Error> {
    let mut i = iterable.iter();
    if let Some(first) = i.next() {
        write!(f, "{}", first)?;
        while let Some(other) = i.next() {
            write!(f, "{}", sep)?;
            write!(f, "{}", other)?;
        }
    }
    Result::Ok(())
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


enum Partial<PlaceHolder, Final> {
    Pending(PlaceHolder),
    Complete(Final)
}

impl<PH : Display,F: Display> Display for Partial<PH, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            Partial::Pending(x) => write!(f, "?{}", x),
            Partial::Complete(x) => write!(f, "{}", x),
        }
    }
}


type ParameterizedExpr<Param,Atom> = Expr<Partial<Param,Atom>>;




struct ChronicleTemplate {
    params: Vec<String>,
    cond: Expr<Partial<usize, Expr<String>>>
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let add = Expr::atom("ADD".to_string());
        let a = Expr::atom("A".to_string());
        let b = Expr::atom("_B".to_string());
        let e = Expr::new(vec![a,b]);
        let e2 = Expr::new(vec!(add, e));

        let partial = e2.map(|s| if s.starts_with("_B") {
            Partial::Pending(s.clone())
        } else {
            Partial::Complete(s.clone())
        }
        );

        println!("{}", e2);
        println!("partial: {}", partial);

    }

}