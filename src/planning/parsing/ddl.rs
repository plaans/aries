
use std::fmt::{Display, Formatter, Error};

use crate::planning::parsing::sexpr::*;
use crate::planning::utils::disp_iter;

pub fn parse_pddl_domain(pb: &str) -> Result<Domain, String> {
    let expr = parse(pb)?;
    read_xddl_domain(expr, Language::PDDL)
}
pub fn parse_pddl_problem(pb: &str) -> Result<Problem, String> {
    let expr = parse(pb)?;
    read_xddl_problem(expr, Language::PDDL)
}


#[derive(Default, Debug, Clone)]
pub struct Domain {
    pub name: String,
    pub types: Vec<Tpe>,
    pub predicates: Vec<Pred>,
    pub tasks: Vec<Task>,
    pub actions: Vec<Action>
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
pub struct Tpe {
    pub name: String,
    pub parent: String
}
impl Display for Tpe {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{} <- {}", self.name, self.parent)
    }
}

#[derive(Debug,Clone)]
pub struct Arg {
    pub name: String,
    pub tpe: String
}

impl Display for Arg {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}: {}", self.name, self.tpe)
    }
}

#[derive(Debug,Clone)]
pub struct Pred {
    pub name: String,
    pub args: Vec<Arg>
}
impl Display for Pred {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(",self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone,Debug)]
pub struct Task {
    pub name: String,
    pub args: Vec<Arg>
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(",self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone,Debug)]
pub struct Action {
    pub name: String,
    pub args: Vec<Arg>,
    pub pre: Expr<String>,
    pub eff: Expr<String>
}

impl Display for Action {
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
                matched.push(es.remove(i).into_sexpr().unwrap());
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
    stream.remove(0).into_atom().ok_or("expected atom".to_string())
}
fn consume_sexpr(stream: &mut Vec<Expr<String>>) -> Result<Vec<Expr<String>>, String> {
    stream.remove(0).into_sexpr().ok_or("expected sexpr".to_string())
}
fn consume_match(stream: &mut Vec<Expr<String>>, symbol: &str) -> Result<(), String> {
    match stream.remove(0) {
        Expr::Leaf(s) if s.as_str() == symbol => Result::Ok(()),
        s => Result::Err(format!("expected {} but got {:?}", symbol, s))
    }
}

fn consume_args(input: &mut Vec<Expr<String>>) -> Result<Vec<Arg>, String> {
    let mut args = Vec::with_capacity(input.len() / 3);
    let mut untyped = Vec::with_capacity(args.len());
    while !input.is_empty() {

        let next = consume_atom(input)?;
        if &next == "-" {
            let tpe = consume_atom(input)?;
            untyped.drain(..)
                .map(|name| Arg { name, tpe: tpe.clone() })
                .for_each(|a| args.push(a));
        } else {
            untyped.push(next);
        }
    }
    // no type given, everything is an object
    untyped.drain(..)
        .map(|name| Arg { name, tpe: "object".to_string() })
        .for_each(|a| args.push(a));
    Result::Ok(args)
}

enum Language {
    HDDL, PDDL
}

fn read_xddl_domain(dom: Expr<String>, _lang: Language) -> Result<Domain, String> {
    let mut res = Domain::default();

    let mut dom = dom.into_sexpr().ok_or("invalid".to_string())?;
    consume_match(&mut dom, "define")?;

    let mut domain_name_decl = dom.remove(0).into_sexpr().ok_or("invalid naming")?;
    consume_match(&mut domain_name_decl, "domain")?;
    res.name = domain_name_decl.remove(0).into_atom().ok_or("missing_name")?;

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
        if !task_block.is_empty() {
            return Result::Err(format!("Unprocessed part of task: {:?}", task_block))
        }

        res.tasks.push(Task { name, args })
    }

    for mut action_block in drain_sub_exprs(&mut dom, ":action") {
        consume_match(&mut action_block, ":action")?;
        let name = consume_atom(&mut action_block)?;
        consume_match(&mut action_block, ":parameters")?;
        let mut args = consume_sexpr(&mut action_block)?;
        let args = consume_args(&mut args)?;

        consume_match(&mut action_block, ":precondition")?;
        let pre = action_block.remove(0);
        consume_match(&mut action_block, ":effect")?;
        let eff = action_block.remove(0);

        if !action_block.is_empty() {
            return Result::Err(format!("Unprocessed part of action: {:?}", action_block))
        }

        res.actions.push(Action { name, args, pre, eff })
    }



    assert!(dom.is_empty(), "Missing unprocessed elements {:?}", dom);


    Result::Ok(res)
}

#[derive(Default, Clone, Debug)]
pub struct Problem {
    pub problem_name: String,
    pub domain_name: String,
    pub objects: Vec<(String, Option<String>)>,
    pub init: Vec<Expr<String>>,
    pub goal: Vec<Expr<String>>
}


fn read_xddl_problem(dom: Expr<String>, _lang: Language) -> Result<Problem, String> {
    let mut res = Problem::default();

    let mut dom = dom.into_sexpr().ok_or("invalid".to_string())?;
    consume_match(&mut dom, "define")?;

    let mut problem_name_block = dom.remove(0).into_sexpr().ok_or("invalid naming")?;
    consume_match(&mut problem_name_block, "problem")?;
    res.problem_name = problem_name_block.remove(0).into_atom().ok_or("missing problem name")?;

    let mut domain_name_decl = dom.remove(0).into_sexpr().ok_or("invalid naming")?;
    consume_match(&mut domain_name_decl, ":domain")?;
    res.domain_name = domain_name_decl.remove(0).into_atom().ok_or("missing domain name")?;

    for mut objects_block in drain_sub_exprs(&mut dom, ":objects") {
        consume_match(&mut objects_block, ":objects")?;
        while !objects_block.is_empty() {
            // push untyped object
            res.objects.push( (consume_atom(&mut objects_block)?, None) )
        }
    }

    for mut inits in drain_sub_exprs(&mut dom, ":init") {
        consume_match(&mut inits, ":init")?;
        res.init.extend_from_slice(&inits);
    }

    for mut goals in drain_sub_exprs(&mut dom, ":goal") {
        consume_match(&mut goals, ":goal")?;
        res.goal.extend_from_slice(&goals);
    }

    assert!(dom.is_empty(), "Missing unprocessed elements {:?}", dom);

    Ok(res)
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn parsing() -> Result<(), String> {
        let prog = "(begin (define r 10) (* pi (* r r)))";
        match parse(prog) {
            Result::Ok(e) => println!("{}", e),
            Result::Err(s) => eprintln!("{}", s)
        }

        Result::Ok(())
    }

    //#[test]
    fn parsing_hddl() -> Result<(), String> {
        let prog = std::fs::read_to_string("problems/hddl/rover-total/domain.hddl")
            .expect("Could not read file");
        match parse(prog.as_str()) {
            Result::Ok(e) => {
                println!("{}", e);

                let dom = read_xddl_domain(e, Language::HDDL).unwrap();

                println!("{}", dom);

            },
            Result::Err(s) => eprintln!("{}", s)
        }

        Result::Ok(())
    }

    #[test]
    fn parsing_pddl_domain() -> Result<(), String> {
        let prog = std::fs::read_to_string("problems/pddl/gripper/domain.pddl")
            .expect("Could not read file");
        match parse(prog.as_str()) {
            Result::Ok(e) => {
                println!("{}", e);

                let dom = read_xddl_domain(e, Language::PDDL).unwrap();

                println!("{}", dom);

            },
            Result::Err(s) => eprintln!("{}", s)
        }

        Result::Ok(())
    }

    #[test]
    fn parsing_pddl_problem() -> Result<(), String> {
        let prog = std::fs::read_to_string("problems/pddl/gripper/problem.pddl")
            .expect("Could not read file");
        match parse(prog.as_str()) {
            Result::Ok(e) => {
                println!("{}", e);

                let _pb = read_xddl_problem(e, Language::PDDL)?;

            },
            Result::Err(s) => eprintln!("{}", s)
        }

        Result::Ok(())
    }
}