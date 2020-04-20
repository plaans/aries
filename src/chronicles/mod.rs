use std::hash::Hash;
use std::fmt::{Display, Formatter, Error};
use std::str::Chars;
use std::any::Any;

pub trait Idx : Into<usize> + Hash + Eq {

}

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

#[derive(Eq, PartialEq, Clone)]
enum Expr<Atom> {
    Leaf(Atom),
    SExpr(Vec<Expr<Atom>>)
}

impl<E : Clone, > Expr<E> {
    pub fn atom(e: E) -> Self {
        Expr::Leaf(e)
    }

    pub fn new(es : Vec<Expr<E>>) -> Self {
        Expr::SExpr(es)
    }

    pub fn map<G, F: Fn(&E) -> G + Copy>(&self, f: F) -> Expr<G> {
        match self {
            Expr::Leaf(a) => Expr::Leaf(f(a)),
            Expr::SExpr(v) => Expr::SExpr(v.iter().map(|e| e.map(f)).collect())
        }
    }

    pub fn as_sexpr(self) -> Option<Vec<Expr<E>>> {
        match self {
            Expr::SExpr(v) => Some(v),
            _ => None
        }
    }

    pub fn as_atom(self) -> Option<E> {
        match self {
            Expr::Leaf(a) => Some(a),
            _ => None
        }
    }
}

fn disp_iter<T: Display>(f: &mut Formatter<'_>, iterable: &[T], sep: &str) -> Result<(),Error> {
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

fn drop_leading_white(mut cur: &str) -> &str {
    while let Some(n) = cur.chars().next() {
        if n.is_whitespace() {
            cur = &cur[1..];
        } else {
            break;
        }
    }
    cur
}

#[derive(Debug,PartialEq)]
enum Token {
    Sym(String),
    LParen,
    RParen
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = s.chars();
    let mut cur = String::new();
    while let Some(n) = chars.next() {
        if n.is_whitespace() || n == '(' || n == ')' {
            if cur.len() > 0 {
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
    println!("{:?}", tokens);
    tokens
}

fn parse(s : &str) -> Result<Expr<String>, String> {
    let tokenized = tokenize(&s);
    let mut tokens = tokenized.iter().peekable();
    read(&mut tokens)
}

fn read(tokens: &mut std::iter::Peekable<core::slice::Iter<Token>>) -> Result<Expr<String>, String> {
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
        },
        Some(Token::RParen) =>  Result::Err("Unexpected closing parenthesis".to_string()),
        None => Result::Err("Unexpected end of output".to_string())
    }
}

#[derive(Default, Debug, Clone)]
struct Domain {
    name: String,
    types: Vec<Tpe>,
    predicates: Vec<Pred>,
    tasks: Vec<Task>
}
impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "# Domain : {}", self.name)?;
        write!(f, "\n# Types \n  ")?;
        disp_iter(f, self.types.as_slice(), "\n  ")?;
        write!(f, "\n# Predicates \n  ")?;
        disp_iter(f, self.predicates.as_slice(), "\n  ")?;
        write!(f, "\n# Tasks \n  ")?;
        disp_iter(f, self.tasks.as_slice(), "\n  ")?;

        Result::Ok(())
    }
}

#[derive(Clone,Debug)]
struct Tpe {
    name: String,
    parent: String
}
impl Display for Tpe {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{} <- {}", self.name, self.parent)
    }
}

#[derive(Debug,Clone)]
struct Arg {
    name: String,
    tpe: String
}

impl Display for Arg {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}: {}", self.name, self.tpe)
    }
}

#[derive(Debug,Clone)]
struct Pred {
    name: String,
    args: Vec<Arg>
}
impl Display for Pred {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(",self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone,Debug)]
struct Task {
    name: String,
    args: Vec<Arg>
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(",self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}



fn drain_sub_exprs<E: Eq + Clone, E2: Into<E>>(es: &mut Vec<Expr<E>>, sym: E2) -> Vec<Vec<Expr<E>>> {
    let head = [Expr::atom(sym.into())];
    let mut matched = Vec::new();
    let mut i = 0;
    while i < es.len() {
        match &es[i] {
            Expr::SExpr(v) if v.starts_with(&head) => {
                matched.push(es.remove(i).as_sexpr().unwrap());
            },
            _ => i += 1
        }
    }
    matched
}

fn sym(s: &str) -> Expr<String> {
    Expr::atom(s.to_string())
}
fn consume_atom(stream: &mut Vec<Expr<String>>) -> Result<String, String> {
    stream.remove(0).as_atom().ok_or("expected atom".to_string())
}
fn consume_sexpr(stream: &mut Vec<Expr<String>>) -> Result<Vec<Expr<String>>, String> {
    stream.remove(0).as_sexpr().ok_or("expected sexpr".to_string())
}
fn consume_match(stream: &mut Vec<Expr<String>>, symbol: &str) -> Result<(), String> {
    match stream.remove(0) {
        Expr::Leaf(s) if s.as_str() == symbol => Result::Ok(()),
        _ => Result::Err("did not match".to_string())
    }
}

fn consume_args(input: &mut Vec<Expr<String>>) -> Result<Vec<Arg>, String> {
    let mut args = Vec::with_capacity(input.len() / 3);
    while !input.is_empty() {
        let name = consume_atom(input)?;
        consume_match(input, "-")?;
        let tpe = consume_atom( input)?;
        args.push(Arg { name: name, tpe: tpe });
    }
    Result::Ok(args)
}

fn read_hddl_domain(dom: Expr<String>) -> Result<Domain, String> {
    let mut res = Domain::default();

    let mut dom = dom.as_sexpr().ok_or("invalid".to_string())?;
    if dom.remove(0) != Expr::atom("define".to_string()) {
        return Result::Err("domaine definition should start with define".to_string())
    }
    let mut domain_name_decl = dom.remove(0).as_sexpr().ok_or("invalid naming")?;
    consume_match(&mut domain_name_decl, "domain")?;

    res.name = domain_name_decl.remove(0).as_atom().ok_or("missing_name")?;


    let types = drain_sub_exprs(&mut dom, ":types".to_string());
    for mut type_block in types {
        consume_match(&mut type_block, ":types")?;
        while !type_block.is_empty() {
            let name = consume_atom(&mut type_block)?;
            consume_match(&mut type_block, "-")?;
            let parent = consume_atom(&mut type_block)?;
            res.types.push(Tpe {name, parent });
        }
    }

    for mut predicate_block in drain_sub_exprs(&mut dom, ":predicates") {
        consume_match(&mut predicate_block, ":predicates")?;
        while !predicate_block.is_empty() {
            let mut pred_decl = consume_sexpr(&mut predicate_block)?;
            let name = consume_atom(&mut pred_decl)?;
            let pred = Pred { name: name, args: consume_args(&mut pred_decl)? };

            res.predicates.push(pred);

        }

    }

    for mut task_block in drain_sub_exprs(&mut dom, ":task") {
        consume_match(&mut task_block, ":task")?;
        let name = consume_atom(&mut task_block)?;
        consume_match(&mut task_block, ":parameters")?;
        let mut args = consume_sexpr(&mut task_block)?;
        let args = consume_args(&mut args)?;
        consume_match(&mut task_block, ":precondition")?;
        if !consume_sexpr(&mut task_block)?.is_empty() {
            return Result::Err("unsupported task preconditions".to_string());
        }
        consume_match(&mut task_block, ":effect")?;
        if !consume_sexpr(&mut task_block)?.is_empty() {
            return Result::Err("unsupported task effects".to_string());
        }
        res.tasks.push(Task {
            name,
            args
        })
    }




    Result::Ok(res)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let ADD = Expr::atom("ADD".to_string());
        let a = Expr::atom("A".to_string());
        let b = Expr::atom("_B".to_string());
        let e = Expr::new(vec![a,b]);
        let e2 = Expr::new(vec!(ADD, e));

        let partial = e2.map(|s| if s.starts_with("_B") {
            Partial::Pending(s.clone())
        } else {
            Partial::Complete(s.clone())
        }
        );

        println!("{}", e2);
        println!("partial: {}", partial);

    }

    #[test]
    fn parsing() -> Result<(), String> {
        let prog = "(begin (define r 10) (* pi (* r r)))";
        match parse(prog) {
            Result::Ok(e) => println!("{}", e),
            Result::Err(s) => eprintln!("{}", s)
        }

        Result::Ok(())
    }

    #[test]
    fn parsing_hddl() -> Result<(), String> {
        let prog = std::fs::read_to_string("problems/hddl/total/rover/domains/rover-domain.hddl")
            .expect("Could not read file");
        match parse(prog.as_str()) {
            Result::Ok(e) => {
                println!("{}", e);

                let dom = read_hddl_domain(e).expect("oups");

                println!("{}", dom);

            },
            Result::Err(s) => eprintln!("{}", s)
        }

        Result::Ok(())
    }

}