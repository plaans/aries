#![allow(dead_code)] // TODO: remove once we exploit the code for HDDL

use anyhow::Context;
use std::fmt::{Display, Error, Formatter};

use crate::parsing::sexpr::*;
use anyhow::*;
use aries_utils::disp_iter;
use aries_utils::input::*;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub fn parse_pddl_domain(pb: Input) -> Result<Domain> {
    let expr = parse(pb)?;
    read_domain(expr).context("Invalid domain")
}
pub fn parse_pddl_problem(pb: Input) -> Result<Problem> {
    let expr = parse(pb)?;
    read_problem(expr).context("Invalid problem")
}

/// Attempts to find the corresponding domain file for the given PDDL/HDDL problem.
/// This method will look for a file named `domain.pddl` (resp. `domain.hddl`) in the
/// current and parent folders.
pub fn find_domain_of(problem_file: &std::path::Path) -> anyhow::Result<PathBuf> {
    // these are the domain file names that we will look for in the current and parent directory
    let mut candidate_domain_files = Vec::with_capacity(2);

    // add domain.pddl or domain.hddl
    candidate_domain_files.push(match problem_file.extension() {
        Some(ext) => Path::new("domain").with_extension(ext),
        None => Path::new("domain.pddl").to_path_buf(),
    });

    let problem_filename = problem_file
        .file_name()
        .context("Invalid file")?
        .to_str()
        .context("Could not convert file name to utf8")?;

    // if the problem file is of the form XXXXX.YY.pb.Zddl or XXXXX.pb.Zddl,
    // then add XXXXX.dom.Zddl to the candidate filenames
    let re = Regex::new("([^\\.]+)(\\.[^\\.]+)?\\.pb\\.([hp]ddl)").unwrap();
    for m in re.captures_iter(problem_filename) {
        let name = format!("{}.dom.{}", &m[1], &m[3]);
        candidate_domain_files.push(name.into());
    }
    // if the problem file is of the form XXXXX.Zddl
    // then add XXXXX-domain.Zddl to the candidate filenames
    let re = Regex::new("([^\\.]+)\\.([hp]ddl)").unwrap();
    for m in re.captures_iter(problem_filename) {
        let name = format!("{}-domain.{}", &m[1], &m[2]);
        candidate_domain_files.push(name.into());
    }

    // directories where to look for the domain
    let mut candidate_directories = Vec::with_capacity(2);
    if let Some(curr) = problem_file.parent() {
        candidate_directories.push(curr);
        if let Some(parent) = curr.parent() {
            candidate_directories.push(parent);
        }
    }

    for f in &candidate_domain_files {
        for &dir in &candidate_directories {
            let candidate = dir.join(f);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    bail!(
        "Could not find find a corresponding file in same or parent directory as the problem file. Candidates: {:?}",
        candidate_domain_files
    );
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PddlFeature {
    Strips,
    Typing,
    Equality,
    NegativePreconditions,
    Hierarchy,
    MethodPreconditions,
    DurativeAction,
    Fluents,
}
impl std::str::FromStr for PddlFeature {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ":strips" => Ok(PddlFeature::Strips),
            ":typing" => Ok(PddlFeature::Typing),
            ":equality" => Ok(PddlFeature::Equality),
            ":negative-preconditions" => Ok(PddlFeature::NegativePreconditions),
            ":hierarchy" => Ok(PddlFeature::Hierarchy),
            ":method-preconditions" => Ok(PddlFeature::MethodPreconditions),
            ":durative-actions" => Ok(PddlFeature::DurativeAction),
            ":fluents" => Ok(PddlFeature::Fluents),
            _ => Err(format!("Unknown feature `{}`", s)),
        }
    }
}
impl Display for PddlFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            PddlFeature::Strips => ":strips",
            PddlFeature::Typing => ":typing",
            PddlFeature::Equality => ":equality",
            PddlFeature::NegativePreconditions => ":negative-preconditions",
            PddlFeature::Hierarchy => ":hierarchy",
            PddlFeature::MethodPreconditions => ":method-preconditions",
            PddlFeature::DurativeAction => ":durative-action",
            PddlFeature::Fluents => ":fluents",
        };
        write!(f, "{}", formatted)
    }
}

#[derive(Debug, Clone)]
pub struct Domain {
    pub name: Sym,
    pub features: Vec<PddlFeature>,
    pub types: Vec<TypedSymbol>,
    pub constants: Vec<TypedSymbol>,
    pub predicates: Vec<Predicate>,
    pub functions: Vec<Function>,
    pub tasks: Vec<TaskDef>,
    pub methods: Vec<Method>,
    pub actions: Vec<Action>,
    pub durative_actions: Vec<DurativeAction>,
}
impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "# Domain : {}", self.name)?;
        write!(f, "\n# Types \n  ")?;
        disp_iter(f, self.types.as_slice(), "\n  ")?;
        write!(f, "\n# Predicates \n  ")?;
        disp_iter(f, self.predicates.as_slice(), "\n  ")?;
        write!(f, "\n# Functions \n  ")?;
        disp_iter(f, self.functions.as_slice(), "\n  ")?;
        write!(f, "\n# Tasks \n  ")?;
        disp_iter(f, self.tasks.as_slice(), "\n  ")?;
        write!(f, "\n# Methods \n  ")?;
        disp_iter(f, self.methods.as_slice(), "\n  ")?;
        write!(f, "\n# Actions \n  ")?;
        disp_iter(f, self.actions.as_slice(), "\n  ")?;
        write!(f, "\n# Durative Actions \n  ")?;
        disp_iter(f, self.durative_actions.as_slice(), "\n  ")?;

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
    pub name: Sym,
    pub args: Vec<TypedSymbol>,
}
impl Display for Predicate {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Sym,
    pub args: Vec<TypedSymbol>,
}
impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name)?;
        disp_iter(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: Sym,
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

type TaskId = Sym;

/// A task, as it appears in a task network.
#[derive(Clone, Debug)]
pub struct Task {
    /// Optional identifier of the task. This identifier is typically used to
    /// refer to the task in ordering constraints.
    pub id: Option<TaskId>,
    pub name: Sym,
    pub arguments: Vec<Sym>,
    source: Option<Loc>,
}
impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} ", self.name)?;
        disp_iter(f, &self.arguments, " ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct Method {
    pub name: Sym,
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
#[derive(Clone, Debug)]
pub struct Ordering {
    pub first_task_id: TaskId,
    pub second_task_id: TaskId,
    source: Option<Loc>,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub name: Sym,
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

#[derive(Clone, Debug)]
pub struct DurativeAction {
    pub name: Sym,
    pub args: Vec<TypedSymbol>,
    pub duration: SExpr,
    pub conditions: Vec<SExpr>,
    pub effects: Vec<SExpr>,
}

impl Display for DurativeAction {
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

/// Returns a localized error on `expr` if the given feature is not present in the domain.
fn check_feature_presence(feature: PddlFeature, domain: &Domain, expr: &SExpr) -> Result<(), ErrLoc> {
    if domain.features.contains(&feature) {
        Ok(())
    } else {
        Err(expr.invalid(format!("Requires the {} feature in the requirements.", feature)))
    }
}

fn read_domain(dom: SExpr) -> std::result::Result<Domain, ErrLoc> {
    let dom = &mut dom.as_list_iter().ok_or_else(|| dom.invalid("Expected a list"))?;

    dom.pop_known_atom("define")?;

    // extract the name of the domain, of the form `(domain XXX)`
    let mut domain_name_decl = dom.pop_list()?.iter();
    domain_name_decl.pop_known_atom("domain")?;
    let name = domain_name_decl.pop_atom().ctx("missing name of domain")?.clone();

    let mut res = Domain {
        name,
        features: vec![],
        types: vec![],
        constants: vec![],
        predicates: vec![],
        functions: vec![],
        tasks: vec![],
        methods: vec![],
        actions: vec![],
        durative_actions: vec![],
    };

    for current in dom {
        // a property associates a key (e.g. `:predicates`) to a value or a sequence of values
        let mut property = current
            .as_list_iter()
            .ok_or_else(|| current.invalid("expected a property list"))?;

        match property.pop_atom()?.as_str() {
            ":requirements" => {
                for feature in property {
                    let feature = feature
                        .as_atom()
                        .ok_or_else(|| feature.invalid("Expected feature name but got list"))?;
                    let f = PddlFeature::from_str(feature.as_str()).map_err(|e| feature.invalid(e))?;

                    res.features.push(f);
                }
            }
            ":predicates" => {
                for pred in property {
                    let mut pred = pred.as_list_iter().ok_or_else(|| pred.invalid("Expected a list"))?;
                    let name = pred.pop_atom()?.clone();
                    let args = consume_typed_symbols(&mut pred)?;
                    res.predicates.push(Predicate { name, args });
                }
            }
            ":types" => {
                if !res.types.is_empty() {
                    return Err(current.invalid("More than one ':types' section definition"));
                }
                let types = consume_typed_symbols(&mut property)?;
                res.types = types;
            }
            ":constants" => {
                if !res.constants.is_empty() {
                    return Err(current.invalid("More than one ':constants' section definition"));
                }
                let constants = consume_typed_symbols(&mut property)?;
                res.constants = constants;
            }
            ":functions" => {
                for func in property {
                    let mut func = func.as_list_iter().ok_or_else(|| func.invalid("Expected a list"))?;
                    let name = func.pop_atom()?.clone();
                    let args = consume_typed_symbols(&mut func)?;
                    res.functions.push(Function { name, args });
                }
            }
            ":action" => {
                let name = property.pop_atom()?.clone();
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
            ":durative-action" => {
                let name = property.pop_atom()?.clone();
                let mut args = Vec::new();
                let mut duration = SExpr::new(&SExpr::Atom(name.clone())); //FIXME:This initialization is correct?
                let mut conditions = Vec::new();
                let mut effects = Vec::new();
                while !property.is_empty() {
                    let key_expr = property.pop_atom()?;
                    let key_loc = key_expr.loc();
                    let key = key_expr.to_string();
                    let value = property.pop().ctx(format!("No value associated to arg: {}", key))?;
                    match key.as_str() {
                        ":parameters" => {
                            if !args.is_empty() {
                                return Err(key_loc.invalid("Duplicated ':parameters' tag is not allowed twice"));
                            }
                            let mut value = value
                                .as_list_iter()
                                .ok_or_else(|| value.invalid("Expected a parameter list"))?;
                            for a in consume_typed_symbols(&mut value)? {
                                args.push(a);
                            }
                        }
                        ":duration" => {
                            duration = value.clone();
                        }
                        ":condition" => {
                            conditions.push(value.clone());
                        }
                        ":effect" => {
                            effects.push(value.clone());
                        }
                        _ => return Err(key_loc.invalid(format!("unsupported key in action: {}", key))),
                    }
                }
                let durative_action = DurativeAction {
                    name,
                    args,
                    duration,
                    conditions,
                    effects,
                };
                res.durative_actions.push(durative_action)
            }
            ":task" => {
                check_feature_presence(PddlFeature::Hierarchy, &res, current)?;
                let name = property.pop_atom().ctx("Missing task name")?.clone();
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
                check_feature_presence(PddlFeature::Hierarchy, &res, current)?;
                let name = property.pop_atom().ctx("Missing task name")?.clone();
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
                    return Err(key_loc.invalid("More than one set of ordered tasks."));
                }
                let value = key_values.pop()?;
                tn.ordered_tasks = parse_conjunction(value, |e| parse_task(e, true))?;
            }
            ":tasks" | ":subtasks" => {
                if !tn.unordered_tasks.is_empty() {
                    return Err(key_loc.invalid("More than one set of unordered tasks."));
                }
                let value = key_values.pop()?;
                tn.unordered_tasks = parse_conjunction(value, |e| parse_task(e, true))?;
            }
            ":ordering" => {
                if !tn.orderings.is_empty() {
                    return Err(key_loc.invalid("More than one set of ordering constraints."));
                }
                let value = key_values.pop()?;
                // parser for a single ordering '(< ID1 ID2)'
                let ordering_parser = |e: &SExpr| {
                    let mut l = e
                        .as_list_iter()
                        .ok_or_else(|| e.invalid("Expected ordering constraint of the form: '(< ID1 ID2)`"))?;
                    l.pop_known_atom("<")?;
                    let first_task_id = l.pop_atom()?.clone();
                    let second_task_id = l.pop_atom()?.clone();
                    if let Some(unexpected) = l.next() {
                        return Err(unexpected.invalid("Expected end of list"));
                    }
                    Ok(Ordering {
                        first_task_id,
                        second_task_id,
                        source: Some(l.loc()),
                    })
                };
                tn.orderings = parse_conjunction(value, ordering_parser)?;
            }
            ":parameters" => {
                let value = key_values.pop_list()?;
                if !value.iter().is_empty() {
                    return Err(value.invalid("No support yet for non-empty parameter lists in task networks."));
                }
            }
            ":constraints" => {
                let value = key_values.pop_list()?;
                if !value.iter().is_empty() {
                    return Err(value.invalid("No support yet for non-empty constraint lists in task networks."));
                }
            }
            _ => return Err(key_loc.invalid("Unsupported keyword in task network")),
        }
    }
    Ok(tn)
}

fn parse_task(e: &SExpr, allow_id: bool) -> std::result::Result<Task, ErrLoc> {
    let mut list = e.as_list_iter().ok_or_else(|| e.invalid("Expected a task name"))?;
    let head = list.pop_atom()?.clone();
    match list.peek() {
        Some(_first_param @ SExpr::Atom(_)) => {
            // start of parameters
            let mut args = Vec::with_capacity(list.len());
            for arg in list {
                let param = arg.as_atom().ok_or_else(|| arg.invalid("Invalid task parameter"))?;
                args.push(param.into());
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

#[derive(Clone, Debug)]
pub struct Problem {
    pub problem_name: Sym,
    pub domain_name: Sym,
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

fn read_problem(problem: SExpr) -> std::result::Result<Problem, ErrLoc> {
    let mut problem = problem
        .as_list_iter()
        .ok_or_else(|| problem.invalid("Expected a list"))?;
    problem.pop_known_atom("define")?;

    let mut problem_name = problem
        .pop_list()
        .ctx("Expected problem name definition of the form '(problem XXXXXX)'")?
        .iter();
    problem_name.pop_known_atom("problem")?;
    let problem_name = problem_name.pop_atom()?.clone();

    let mut domain_name_def = problem.pop_list()?.iter();
    domain_name_def.pop_known_atom(":domain")?;
    let domain_name = domain_name_def.pop_atom()?.clone();

    let mut res = Problem {
        problem_name,
        domain_name,
        objects: vec![],
        init: vec![],
        task_network: None,
        goal: vec![],
    };

    for current in problem {
        // a property associates a key (e.g. `:objects`) to a value or a sequence of values
        let mut property = current
            .as_list_iter()
            .ok_or_else(|| current.invalid("Expected a list"))?;
        match property.pop_atom()?.as_str() {
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
        let source = "../problems/hddl/towers/domain.hddl";
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
