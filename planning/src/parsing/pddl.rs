#![allow(dead_code)] // TODO: remove once we exploit the code for HDDL

use anyhow::Context;
use std::fmt::{Display, Error, Formatter};

use crate::parsing::sexpr::*;
use anyhow::*;
use aries_utils::disp_iter;
use aries_utils::input::*;
use std::str::FromStr;

pub fn parse_pddl_domain(pb: Input) -> Result<Domain> {
    let expr = parse(pb)?;
    read_domain(expr).context("Invalid domain")
}
pub fn parse_pddl_problem(pb: Input) -> Result<Problem> {
    let expr = parse(pb)?;
    read_problem(expr).context("Invalid problem")
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PddlFeature {
    Strips,
    Typing,
    NegativePreconditions,
    Hierarchy,
    MethodPreconditions,
}
impl std::str::FromStr for PddlFeature {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ":strips" => Ok(PddlFeature::Strips),
            ":typing" => Ok(PddlFeature::Typing),
            ":negative-preconditions" => Ok(PddlFeature::NegativePreconditions),
            ":hierarchy" => Ok(PddlFeature::Hierarchy),
            ":method-preconditions" => Ok(PddlFeature::MethodPreconditions),
            _ => Err(format!("Unknown feature `{}`", s)),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Domain {
    pub name: String,
    pub features: Vec<PddlFeature>,
    pub types: Vec<TypedSymbol>,
    pub predicates: Vec<Predicate>,
    pub tasks: Vec<TaskDef>,
    pub methods: Vec<Method>,
    pub actions: Vec<Action>,
}
impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "# Domain : {}", self.name)?;
        write!(f, "\n# Types \n  ")?;
        disp_iter(f, self.types.as_slice(), "\n  ")?;
        write!(f, "\n# Predicates \n  ")?;
        disp_iter(f, self.predicates.as_slice(), "\n  ")?;
        write!(f, "\n# Tasks \n  ")?;
        disp_iter(f, self.tasks.as_slice(), "\n  ")?;
        write!(f, "\n# Methods \n  ")?;
        disp_iter(f, self.methods.as_slice(), "\n  ")?;
        write!(f, "\n# Actions \n  ")?;
        disp_iter(f, self.actions.as_slice(), "\n  ")?;

        Result::Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Tpe {
    pub name: Sym,
    pub parent: Sym,
}
impl Display for Tpe {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{} <- {}", self.name, self.parent)
    }
}

#[derive(Debug, Clone)]
pub struct TypedSymbol {
    pub symbol: Sym,
    pub tpe: Option<Sym>,
}
impl TypedSymbol {
    pub fn new(symbol: impl Into<Sym>, tpe: impl Into<Sym>) -> TypedSymbol {
        TypedSymbol {
            symbol: symbol.into(),
            tpe: Some(tpe.into()),
        }
    }
}

impl Display for TypedSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match &self.tpe {
            Some(tpe) => write!(f, "{}: {}", self.symbol, tpe),
            None => write!(f, "{}", self.symbol),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Predicate {
    pub name: String,
    pub args: Vec<TypedSymbol>,
}
impl Display for Predicate {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: String,
    pub args: Vec<TypedSymbol>,
    source: Option<Loc>,
}

impl Display for TaskDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

type TaskId = String;

/// A task, as it appears in a task network.
#[derive(Clone, Default, Debug)]
pub struct Task {
    /// Optional identifier of the task. This identifier is typically used to
    /// refer to the task in ordering constraints.
    pub id: Option<TaskId>,
    pub name: String,
    pub arguments: Vec<String>,
    source: Option<Loc>,
}
impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} ", self.name)?;
        disp_iter(f, &self.arguments, " ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Default, Debug)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<TypedSymbol>,
    pub task: Task,
    pub precondition: Vec<SExpr>,
    pub subtask_network: TaskNetwork,
    source: Option<Loc>,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone, Default, Debug)]
pub struct TaskNetwork {
    pub ordered_tasks: Vec<Task>,
    pub unordered_tasks: Vec<Task>,
    pub orderings: Vec<Ordering>,
}

/// Constraint specifying that the task identified by `first_task_id` should end
/// before the one identified by `second_task_id`
#[derive(Clone, Default, Debug)]
pub struct Ordering {
    pub first_task_id: TaskId,
    pub second_task_id: TaskId,
    source: Option<Loc>,
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
/// Consume a typed list of symbols
///  - (a - loc b - loc c - loc) : symbols a, b and c of type loc
///  - (a b c - loc)  : symbols a, b and c of type loc
///  - (a b c) : symbols a b and c of type object
fn consume_typed_symbols(input: &mut ListIter) -> std::result::Result<Vec<TypedSymbol>, ErrLoc> {
    let mut args = Vec::with_capacity(input.len() / 3);
    let mut untyped: Vec<Sym> = Vec::with_capacity(args.len());
    while !input.is_empty() {
        let next = input.pop_atom()?;
        if next.as_str() == "-" {
            let tpe = input.pop_atom()?;
            untyped
                .drain(..)
                .map(|name| TypedSymbol::new(name, tpe))
                .for_each(|a| args.push(a));
        } else {
            untyped.push(next.into());
        }
    }
    // no type given, everything is an object
    untyped
        .drain(..)
        .map(|name| TypedSymbol {
            symbol: name,
            tpe: None,
        })
        .for_each(|a| args.push(a));
    Result::Ok(args)
}

fn read_domain(dom: SExpr) -> std::result::Result<Domain, ErrLoc> {
    let mut res = Domain::default();

    let dom = &mut dom.as_list_iter().ok_or_else(|| dom.invalid("Expected a list"))?;

    dom.pop_known_atom("define")?;

    // extract the name of the domain, of the form `(domain XXX)`
    let mut domain_name_decl = dom.pop_list()?.iter();
    domain_name_decl.pop_known_atom("domain")?;
    res.name = domain_name_decl.pop_atom().ctx("missing name of domain")?.to_string();

    for current in dom {
        // a property associates a key (e.g. `:predicates`) to a value or a sequence of values
        let mut property = current
            .as_list_iter()
            .ok_or_else(|| current.invalid("expected a property list"))?;

        match property.pop_atom()?.as_str() {
            ":requirements" => {
                while let Some(feature) = property.next() {
                    let feature = feature
                        .as_atom()
                        .ok_or_else(|| feature.invalid("Expected feature name but got list"))?;
                    let f = PddlFeature::from_str(feature.as_str()).map_err(|e| feature.invalid(e))?;

                    res.features.push(f);
                }
            }
            ":predicates" => {
                while let Some(pred) = property.next() {
                    let mut pred = pred.as_list_iter().ok_or_else(|| pred.invalid("Expected a list"))?;
                    let name = pred.pop_atom()?.to_string();
                    let args = consume_typed_symbols(&mut pred)?;
                    res.predicates.push(Predicate { name, args });
                }
            }
            ":types" => {
                if !res.types.is_empty() {
                    return Err(current.invalid("More than one types defintion"));
                }
                let types = consume_typed_symbols(&mut property)?;
                res.types = types;
            }
            ":action" => {
                let name = property.pop_atom()?.to_string();
                let mut args = Vec::new();
                let mut pre = Vec::new();
                let mut eff = Vec::new();
                while !property.is_empty() {
                    let key_expr = property.pop_atom()?;
                    let key_loc = key_expr.loc();
                    let key = key_expr.to_string();
                    let value = property.pop().ctx(format!("No value associated to arg: {}", key))?;
                    match key.as_str() {
                        ":parameters" => {
                            let mut value = value
                                .as_list_iter()
                                .ok_or_else(|| value.invalid("Expected a parameter list"))?;
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
                        _ => return Err(key_loc.invalid(format!("unsupported key in action: {}", key))),
                    }
                }
                res.actions.push(Action { name, args, pre, eff })
            }
            ":task" => {
                let name = property.pop_atom().ctx("Missing task name")?.to_string();
                property.pop_known_atom(":parameters")?;
                let params = property.pop_list().ctx("Expected a parameter list")?;
                let params = consume_typed_symbols(&mut params.iter())?;
                let task = TaskDef {
                    name,
                    args: params,
                    source: Some(current.loc().clone()),
                };
                res.tasks.push(task);
            }
            ":method" => {
                let name = property.pop_atom().ctx("Missing task name")?.to_string();
                property.pop_known_atom(":parameters")?;
                let params = property.pop_list().ctx("Expected a parameter list")?;
                let parameters = consume_typed_symbols(&mut params.iter())?;
                property.pop_known_atom(":task")?;
                let task = parse_task(property.pop()?, false)?;
                let precondition = if property.peek().map_or(false, |e| e.is_atom(":precondition")) {
                    property.pop_known_atom(":precondition").unwrap();
                    vec![property.pop()?.clone()]
                } else {
                    Vec::new()
                };
                let method = Method {
                    name,
                    parameters,
                    task,
                    precondition,
                    subtask_network: parse_task_network(property)?,
                    source: Some(current.loc()),
                };
                res.methods.push(method);
            }

            _ => return Err(current.invalid("unsupported block")),
        }
    }
    Ok(res)
}

fn parse_task_network(mut key_values: ListIter) -> R<TaskNetwork> {
    let mut tn = TaskNetwork::default();
    while !key_values.is_empty() {
        let key = key_values.pop_atom()?;
        let key_loc = key.loc();
        match key.as_str() {
            ":ordered-tasks" | ":ordered-subtasks" => {
                if !tn.ordered_tasks.is_empty() {
                    return Err(key_loc.invalid("More than on set of ordered tasks."));
                }
                let value = key_values.pop()?;
                let subtasks = parse_conjunction(value, |e| parse_task(e, true))?;
                tn.ordered_tasks = subtasks;
            }
            _ => return Err(key_loc.invalid("Unsupported keyword in task network")),
        }
    }
    Ok(tn)
}

fn parse_task(e: &SExpr, allow_id: bool) -> std::result::Result<Task, ErrLoc> {
    let mut list = e.as_list_iter().ok_or_else(|| e.invalid("Expected a task name"))?;
    let head = list.pop_atom()?.to_string();
    match list.peek() {
        Some(_first_param @ SExpr::Atom(_)) => {
            // start of parameters
            let mut args = Vec::with_capacity(list.len());
            for arg in list {
                let param = arg.as_atom().ok_or_else(|| arg.invalid("Invalid task parameter"))?;
                args.push(param.to_string());
            }
            Ok(Task {
                id: None,
                name: head,
                arguments: args,
                source: Some(e.loc()),
            })
        }
        Some(task @ SExpr::List(_)) => {
            if allow_id {
                let mut task = parse_task(task, false)?;
                task.id = Some(head);
                task.source = Some(e.loc());
                Ok(task)
            } else {
                Err(e.invalid("Expected a task (without an id)"))
            }
        }
        None => {
            // this is a parameter-less task
            Ok(Task {
                id: None,
                name: head,
                arguments: vec![],
                source: Some(e.loc()),
            })
        }
    }
}

type R<T> = std::result::Result<T, ErrLoc>;

/// given a term type T, parse one of `T, () or (and T T ...)
fn parse_conjunction<T>(e: &SExpr, item_parser: impl Fn(&SExpr) -> R<T>) -> R<Vec<T>> {
    match e {
        SExpr::Atom(_) => Ok(vec![item_parser(e)?]),
        SExpr::List(l) => {
            if let Some(conjuncts) = e.as_application("and") {
                let mut result = Vec::with_capacity(conjuncts.len());
                for c in conjuncts {
                    result.push(item_parser(c)?);
                }
                Ok(result)
            } else if l.iter().is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![item_parser(e)?])
            }
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Problem {
    pub problem_name: String,
    pub domain_name: String,
    pub objects: Vec<TypedSymbol>,
    pub init: Vec<SExpr>,
    pub task_network: Option<TaskNetwork>,
    pub goal: Vec<SExpr>,
}

impl Display for Problem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "# Problem {} (domain: {})", &self.problem_name, &self.domain_name)?;
        write!(f, "\n# Objects \n  ")?;
        disp_iter(f, self.objects.as_slice(), "\n  ")?;
        write!(f, "\n# Init \n  ")?;
        disp_iter(f, self.init.as_slice(), "\n  ")?;
        write!(f, "\n# Goal \n  ")?;
        disp_iter(f, self.goal.as_slice(), "\n  ")?;
        if let Some(tn) = &self.task_network {
            write!(f, "\n# Tasks \n")?;
            for task in tn.ordered_tasks.iter().chain(tn.unordered_tasks.iter()) {
                writeln!(f, "  {}", task)?;
            }
        }

        Result::Ok(())
    }
}

fn read_problem(dom: SExpr) -> std::result::Result<Problem, ErrLoc> {
    let mut res = Problem::default();

    let mut dom = dom.as_list_iter().ok_or_else(|| dom.invalid("Expected a list"))?;
    dom.pop_known_atom("define")?;

    let mut problem_name = dom
        .pop_list()
        .ctx("Expected problem name definition of the form '(problem XXXXXX)'")?
        .iter();
    problem_name.pop_known_atom("problem")?;
    res.problem_name = problem_name.pop_atom()?.to_string();

    for current in dom {
        // a property associates a key (e.g. `:objects`) to a value or a sequence of values
        let mut property = current
            .as_list_iter()
            .ok_or_else(|| current.invalid("Expected a list"))?;
        match property.pop_atom()?.as_str() {
            ":domain" => {
                res.domain_name = property.pop_atom().ctx("Expected domain name")?.to_string();
            }
            ":objects" => {
                let objects = consume_typed_symbols(&mut property)?;
                for o in objects {
                    res.objects.push(o);
                }
            }
            ":init" => {
                for fact in property {
                    res.init.push(fact.clone());
                }
            }
            ":goal" => {
                for goal in property {
                    res.goal.push(goal.clone());
                }
            }
            ":htn" => {
                if res.task_network.is_some() {
                    return Err(current.invalid("More than one task network specified"));
                }
                res.task_network = Some(parse_task_network(property)?);
            }
            _ => return Err(current.invalid("unsupported block")),
        }
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parsing() -> Result<(), String> {
        let prog = "(begin (define r 10) (* pi (* r r)))";
        match parse(prog) {
            Result::Ok(e) => println!("{}", e),
            Result::Err(s) => eprintln!("{}", s),
        }

        Result::Ok(())
    }

    #[test]
    fn parsing_hddl() -> Result<()> {
        let source = "../problems/hddl/total-order/Towers/domain.hddl";
        let source = PathBuf::from_str(&source)?;
        let source = Input::from_file(&source)?;

        match parse(source) {
            Result::Ok(e) => {
                println!("{}", e);

                let dom = match read_domain(e) {
                    Ok(dom) => dom,
                    Err(e) => {
                        eprintln!("{}", &e);
                        bail!("Could not parse")
                    }
                };

                println!("{}", dom);
            }
            Result::Err(s) => eprintln!("{}", s),
        }

        Result::Ok(())
    }
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
