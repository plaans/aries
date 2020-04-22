



#[derive(Eq, PartialEq, Clone)]
pub enum Expr<Atom> {
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


#[derive(Debug,PartialEq)]
enum Token {
    Sym(String),
    LParen,
    RParen
}

pub fn parse(s : &str) -> Result<Expr<String>, String> {
    let tokenized = tokenize(&s);
    let mut tokens = tokenized.iter().peekable();
    read(&mut tokens)
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

