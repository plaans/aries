#![allow(dead_code)] // TODO: remove once we exploit the code for HDDL

use std::fmt::{Display, Error, Formatter};

use crate::parsing::sexpr::*;
use anyhow::*;
use aries_utils::disp_iter;
use std::collections::HashSet;
use std::str::FromStr;

pub fn parse_pddl_domain(pb: &str) -> Result<Domain> {
    let expr = parse(pb)?;
    read_xddl_domain(expr, Language::PDDL)
}
pub fn parse_pddl_problem(pb: &str) -> Result<Problem> {
    let expr = parse(pb)?;
    read_xddl_problem(expr, Language::PDDL)
}

#[derive(Debug, Copy, Clone)]
pub enum PddlFeature {
    Strips,
    Typing,
}
impl std::str::FromStr for PddlFeature {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ":strips" => Ok(PddlFeature::Strips),
            ":typing" => Ok(PddlFeature::Typing),
            _ => Err(()),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Domain {
    pub name: String,
    pub features: Vec<PddlFeature>,
    pub types: Vec<Tpe>,
    pub predicates: Vec<Pred>,
    pub tasks: Vec<Task>,
    pub actions: Vec<Action>,
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

#[derive(Clone, Debug)]
pub struct Tpe {
    pub name: String,
    pub parent: String,
}
impl Display for Tpe {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{} <- {}", self.name, self.parent)
    }
}

#[derive(Debug, Clone)]
pub struct TypedSymbol {
    pub symbol: String,
    pub tpe: String,
}

impl Display for TypedSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}: {}", self.symbol, self.tpe)
    }
}

#[derive(Debug, Clone)]
pub struct Pred {
    pub name: String,
    pub args: Vec<TypedSymbol>,
}
impl Display for Pred {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub name: String,
    pub args: Vec<TypedSymbol>,
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    pub name: String,
    pub args: Vec<TypedSymbol>,
    pub pre: Vec<Expression>,
    pub eff: Vec<Expression>,
}

#[derive(Clone, Debug)]
pub enum Expression {
    Atom(String),
    List(Vec<Expression>),
}

impl Expression {
    pub fn as_application_args(&self, fun: &str) -> Option<&[Expression]> {
        match self {
            Expression::List(xs) => match xs.as_slice() {
                [Expression::Atom(head), rest @ ..] if head == fun => Some(rest),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Expression]> {
        match self {
            Expression::List(xs) => Some(xs.as_slice()),
            _ => None,
        }
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Expression::Atom(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

impl<'a> From<&SExpr<'a>> for Expression {
    fn from(e: &SExpr<'a>) -> Self {
        if let Some(atom) = e.as_atom() {
            Expression::Atom(atom.to_string())
        } else if let Some(list) = e.as_list() {
            Expression::List(list.iter().map(Expression::from).collect())
        } else {
            unreachable!()
        }
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

// fn drain_sub_exprs<E: Eq + Clone, E2: Into<E>>(es: &mut Vec<Expr<E>>, sym: E2) -> Vec<Vec<Expr<E>>> {
//     // let head = [Expr::atom(sym.into())];
//     // let mut matched = Vec::new();
//     // let mut i = 0;
//     // while i < es.len() {
//     //     match &es[i] {
//     //         Expr::List(v) if v.starts_with(&head) => {
//     //             matched.push(es.remove(i).into_sexpr().unwrap());
//     //         }
//     //         _ => i += 1,
//     //     }
//     // }
//     // matched
//     todo!()
// }

// fn sym(s: &str) -> Expr<String> {
//     // Expr::atom(s.to_string())
//     todo!()
// }
// fn consume_atom(stream: &mut Vec<Expr<String>>) -> Result<String> {
//     // stream.remove(0).into_atom().context("expected atom")
//     todo!()
// }
// fn consume_sexpr(stream: &mut Vec<Expr<String>>) -> Result<Vec<Expr<String>>> {
//     // stream.remove(0).into_sexpr().context("expected sexpr")
//     todo!()
// }
// fn next_matches(stream: &[Expr<String>], symbol: &str) -> bool {
//     // matches!(&stream[0], Expr::Atom(s) if s.as_str() == symbol)
//     todo!()
// }
// fn consume_match(stream: &mut Vec<SExpr>, symbol: &str) -> Result<()> {
//     match stream.pop()
//     // match stream.remove(0) {
//     //     Expr::Atom(s) if s.as_str() == symbol => Result::Ok(()),
//     //     s => bail!("expected {} but got {:?}", symbol, s),
//     // }
//     todo!()
// }

fn consume_typed_symbols(input: &mut ListIter) -> Result<Vec<TypedSymbol>> {
    let mut args = Vec::with_capacity(input.len() / 3);
    let mut untyped = Vec::with_capacity(args.len());
    while !input.is_empty() {
        let next = input.pop_atom()?;
        if next == "-" {
            let tpe = input.pop_atom()?;
            untyped
                .drain(..)
                .map(|name| TypedSymbol {
                    symbol: name,
                    tpe: tpe.to_string(),
                })
                .for_each(|a| args.push(a));
        } else {
            untyped.push(next.to_string());
        }
    }
    // no type given, everything is an object
    untyped
        .drain(..)
        .map(|name| TypedSymbol {
            symbol: name,
            tpe: "object".to_string(),
        })
        .for_each(|a| args.push(a));
    Result::Ok(args)
}

enum Language {
    HDDL,
    PDDL,
}

fn read_xddl_domain<'a>(dom: SExpr<'a>, _lang: Language) -> Result<Domain> {
    let mut res = Domain::default();
    //
    let dom = &mut dom.as_list_iter().context("invalid")?;
    dom.pop_known_atom("define")?;
    // consume_match(dom, "define")?;
    //
    let mut domain_name_decl = dom.pop_list()?;
    // &mut dom.remove(0).into_sexpr().context("invalid naming")?;
    domain_name_decl.pop_known_atom("domain")?;
    // consume_match(domain_name_decl, "domain")?;
    res.name = domain_name_decl.pop_atom().context("missing_name")?.to_string();

    while let Some(current) = dom.next() {
        let mut next = current.as_list_iter().context("got a single atom")?;

        match next.pop_atom()? {
            ":requirements" => {
                while let Some(feature) = next.next() {
                    let feature_str = feature.as_atom().with_context(|| {
                        format!(
                            "Expected feature name but got list:\n{}",
                            feature.display_with_context()
                        )
                    })?;
                    let f = PddlFeature::from_str(feature_str)
                        .ok()
                        .with_context(|| format!("Unkown feature:\n{}", feature.display_with_context()))?;
                    res.features.push(f);
                }
            }
            ":predicates" => {
                while let Some(pred) = next.next() {
                    let mut pred = pred.as_list_iter().context("Expected list")?;
                    let name = pred.pop_atom()?.to_string();
                    let args = consume_typed_symbols(&mut pred)?;
                    res.predicates.push(Pred { name, args });
                }
            }
            ":types" => {
                let types = consume_typed_symbols(&mut next)?;
                for tpe in types {
                    res.types.push(Tpe {
                        name: tpe.tpe,
                        parent: tpe.symbol,
                    })
                }
            }
            ":action" => {
                let name = next.pop_atom()?.to_string();
                let mut args = Vec::new();
                let mut pre = Vec::new();
                let mut eff = Vec::new();
                while !next.is_empty() {
                    let key = next.pop_atom()?.to_string();
                    let value = next
                        .next()
                        .with_context(|| format!("No value associated to arg: {}", key))?;
                    match key.as_str() {
                        ":parameters" => {
                            let mut value = value.as_list_iter().context("Expected a parameter list")?;
                            for a in consume_typed_symbols(&mut value)? {
                                args.push(a);
                            }
                        }
                        ":precondition" => {
                            pre.push(value.into());
                        }
                        ":effect" => {
                            eff.push(value.into());
                        }
                        _ => bail!("unsupported key in action: {}", key),
                    }
                }
                res.actions.push(Action { name, args, pre, eff })
            }

            x => bail!("unsupported block:\n{}", current.display_with_context()),
        }
    }
    Ok(res)
}

#[derive(Default, Clone, Debug)]
pub struct Problem {
    pub problem_name: String,
    pub domain_name: String,
    pub objects: Vec<(String, Option<String>)>,
    pub init: Vec<Expression>,
    pub goal: Vec<Expression>,
}

fn read_xddl_problem(dom: SExpr, _lang: Language) -> Result<Problem> {
    let mut res = Problem::default();

    let mut dom = dom.as_list_iter().context("invalid")?;
    dom.pop_known_atom("define")?;

    let mut problem_name = dom
        .pop_list()
        .context("Expected problem name definition of the form '(problem XXXXXX)'")?;
    problem_name.pop_known_atom("problem")?;
    res.problem_name = problem_name.pop_atom()?.to_string();

    while let Some(current) = dom.next() {
        let mut next = current.as_list_iter().context("got a single atom")?;
        match next.pop_atom()? {
            ":domain" => {
                res.domain_name = next.pop_atom().context("Expected domain name")?.to_string();
            }
            ":objects" => {
                let objects = consume_typed_symbols(&mut next)?;
                for o in objects {
                    res.objects.push((o.symbol, Some(o.tpe)));
                }
            }
            ":init" => {
                while let Some(fact) = next.next() {
                    res.init.push(fact.into());
                }
            }
            ":goal" => {
                while let Some(goal) = next.next() {
                    res.goal.push(goal.into());
                }
            }
            _ => bail!("Unsupported block:\n{}", current.display_with_context()),
        }
    }

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
            Result::Err(s) => eprintln!("{}", s),
        }

        Result::Ok(())
    }

    //#[test]
    // fn parsing_hddl() -> Result<(), String> {
    //     let prog = std::fs::read_to_string("problems/hddl/rover-total/domain.hddl").expect("Could not read file");
    //     match parse(prog.as_str()) {
    //         Result::Ok(e) => {
    //             println!("{}", e);
    //
    //             let dom = read_xddl_domain(e, Language::HDDL).unwrap();
    //
    //             println!("{}", dom);
    //         }
    //         Result::Err(s) => eprintln!("{}", s),
    //     }
    //
    //     Result::Ok(())
    // }
    //
    // #[test]
    // fn parsing_pddl_domain() -> Result<(), String> {
    //     let prog = std::fs::read_to_string("../problems/pddl/gripper/domain.pddl").expect("Could not read file");
    //     match parse(prog.as_str()) {
    //         Result::Ok(e) => {
    //             println!("{}", e);
    //
    //             let dom = read_xddl_domain(e, Language::PDDL).unwrap();
    //
    //             println!("{}", dom);
    //         }
    //         Result::Err(s) => eprintln!("{}", s),
    //     }
    //
    //     Result::Ok(())
    // }
    //
    // #[test]
    // fn parsing_pddl_problem() -> Result<()> {
    //     let prog = std::fs::read_to_string("../problems/pddl/gripper/problem.pddl").expect("Could not read file");
    //     match parse(prog.as_str()) {
    //         Result::Ok(e) => {
    //             println!("{}", e);
    //
    //             let _pb = read_xddl_problem(e, Language::PDDL)?;
    //         }
    //         Result::Err(s) => eprintln!("{}", s),
    //     }
    //
    //     Result::Ok(())
    // }
}
