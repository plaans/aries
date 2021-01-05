#![allow(dead_code)] // TODO: remove once we exploit the code for HDDL

use anyhow::Context;
use std::fmt::{Display, Error, Formatter};

use crate::parsing::sexpr::*;
use anyhow::*;
use aries_utils::disp_iter;
use std::str::FromStr;

pub fn parse_pddl_domain(pb: Source) -> Result<Domain> {
    let expr = parse(pb)?;
    read_xddl_domain(expr, Language::PDDL).context("Invalid domain")
}
pub fn parse_pddl_problem(pb: Source) -> Result<Problem> {
    let expr = parse(pb)?;
    read_xddl_problem(expr, Language::PDDL).context("Invalid problem")
}

#[derive(Debug, Copy, Clone)]
pub enum PddlFeature {
    Strips,
    Typing,
}
impl std::str::FromStr for PddlFeature {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ":strips" => Ok(PddlFeature::Strips),
            ":typing" => Ok(PddlFeature::Typing),
            _ => Err(format!("Unknown feature `{}`", s)),
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
    pub pre: Vec<SExpr>,
    pub eff: Vec<SExpr>,
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

fn consume_typed_symbols(input: &mut ListIter) -> std::result::Result<Vec<TypedSymbol>, ErrLoc> {
    let mut args = Vec::with_capacity(input.len() / 3);
    let mut untyped = Vec::with_capacity(args.len());
    while !input.is_empty() {
        let next = input.pop_atom()?;
        if next.as_str() == "-" {
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

fn read_xddl_domain<'a>(dom: SExpr, _lang: Language) -> std::result::Result<Domain, ErrLoc> {
    let mut res = Domain::default();
    //
    let dom = &mut dom
        .as_list_iter()
        .ok_or("Expected a list")
        .localized(dom.source(), dom.span())?;
    dom.pop_known_atom("define")?;
    // consume_match(dom, "define")?;
    //
    let mut domain_name_decl = dom.pop_list()?.iter();
    // &mut dom.remove(0).into_sexpr().context("invalid naming")?;
    domain_name_decl.pop_known_atom("domain")?;
    // consume_match(domain_name_decl, "domain")?;
    res.name = domain_name_decl.pop_atom().ctx("missing_name")?.to_string();

    while let Some(current) = dom.next() {
        let mut next = current
            .as_list_iter()
            .localized(current.source(), current.span())
            .ctx("got a single atom")?;

        match next.pop_atom()?.as_str() {
            ":requirements" => {
                while let Some(feature) = next.next() {
                    let feature = feature
                        .as_atom()
                        .ok_or("Expected feature name but got list")
                        .localized(feature.source(), feature.span())?;
                    let f = PddlFeature::from_str(feature.as_str()).localized(&feature.source, feature.span())?;

                    res.features.push(f);
                }
            }
            ":predicates" => {
                while let Some(pred) = next.next() {
                    let mut pred = pred
                        .as_list_iter()
                        .localized(pred.source(), pred.span())
                        .ctx("Expected list")?;
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
                    let key_expr = next.pop_atom()?;
                    let key_source = key_expr.source.clone();
                    let key_span = key_expr.span();
                    let key = key_expr.to_string();
                    let value = next.pop().ctx(format!("No value associated to arg: {}", key))?;
                    match key.as_str() {
                        ":parameters" => {
                            let mut value = value
                                .as_list_iter()
                                .localized(value.source(), value.span())
                                .ctx("Expected a parameter list")?;
                            for a in consume_typed_symbols(&mut value)? {
                                args.push(a);
                            }
                        }
                        ":precondition" => {
                            pre.push(value.clone());
                        }
                        ":effect" => {
                            eff.push(value.clone());
                        }
                        _ => {
                            return Err(format!("unsupported key in action: {}", key)).localized(&key_source, key_span)
                        }
                    }
                }
                res.actions.push(Action { name, args, pre, eff })
            }

            _ => return Err("unsupported block").localized(current.source(), current.span()),
        }
    }
    Ok(res)
}

#[derive(Default, Clone, Debug)]
pub struct Problem {
    pub problem_name: String,
    pub domain_name: String,
    pub objects: Vec<(String, Option<String>)>,
    pub init: Vec<SExpr>,
    pub goal: Vec<SExpr>,
}

fn read_xddl_problem(dom: SExpr, _lang: Language) -> std::result::Result<Problem, ErrLoc> {
    let mut res = Problem::default();

    let mut dom = dom.as_list_iter().localized(dom.source(), dom.span()).ctx("invalid")?;
    dom.pop_known_atom("define")?;

    let mut problem_name = dom
        .pop_list()
        .ctx("Expected problem name definition of the form '(problem XXXXXX)'")?
        .iter();
    problem_name.pop_known_atom("problem")?;
    res.problem_name = problem_name.pop_atom()?.to_string();

    while let Some(current) = dom.next() {
        let mut next = current
            .as_list_iter()
            .localized(current.source(), current.span())
            .ctx("Expected a list")?;
        match next.pop_atom()?.as_str() {
            ":domain" => {
                res.domain_name = next.pop_atom().ctx("Expected domain name")?.to_string();
            }
            ":objects" => {
                let objects = consume_typed_symbols(&mut next)?;
                for o in objects {
                    res.objects.push((o.symbol, Some(o.tpe)));
                }
            }
            ":init" => {
                while let Some(fact) = next.next() {
                    res.init.push(fact.clone());
                }
            }
            ":goal" => {
                while let Some(goal) = next.next() {
                    res.goal.push(goal.clone());
                }
            }
            _ => return Err(format!("unsupported block")).localized(current.source(), current.span()),
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
