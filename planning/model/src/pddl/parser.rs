#![allow(dead_code)] // TODO: remove once we exploit the code for HDDL
use crate::Res;
use crate::Sym;
use crate::errors::*;

use itertools::Itertools;
use smallvec::{SmallVec, smallvec};
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::pddl::input::*;
use crate::pddl::sexpr::*;
use crate::utils::disp_slice;
use std::str::FromStr;

pub fn parse_pddl_domain(pb: Input) -> Res<Domain> {
    let pb = Arc::new(pb);
    let expr = parse(pb.clone())?;
    read_domain(expr).title("Invalid domain: Syntax error")
}
pub fn parse_pddl_problem(pb: Input) -> Res<Problem> {
    let pb = Arc::new(pb);
    let expr = parse(pb.clone())?;
    read_problem(expr).title("Invalid problem: Syntax error")
}
pub fn parse_plan(plan: Input) -> Res<Plan> {
    let pb = Arc::new(plan);
    let expr = parse_many(pb.clone())?;
    let mut actions = Vec::with_capacity(expr.len());
    for e in expr {
        let mut elems = e
            .as_list_iter()
            .ok_or_else(|| e.invalid("expected a list with action name and parameters"))?;
        let name = elems.pop_atom()?.clone();
        let mut params = Vec::new();
        while !elems.is_empty() {
            let param = elems.pop_atom()?;
            params.push(param.clone());
        }
        actions.push(ActionInstance {
            name,
            arguments: params,
            span: e.loc(),
        });
    }
    Ok(Plan::ActionSequence(actions))
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PddlFeature {
    Strips,
    Typing,
    Equality,
    NegativePreconditions,
    UniversalPreconditions,
    ExistentialPreconditions,
    QuantifiedPreconditions,
    Hierarchy,
    MethodPreconditions,
    DurativeAction,
    DurationInequalities,
    Fluents,
    NumericFluent,
    ObjectFluent,
    ConditionalEffects,
    TimedInitialLiterals,
    Adl,
    Preferences,
    Constraints,
    ActionCosts,
    GoalUtilities,
}
impl std::str::FromStr for PddlFeature {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ":strips" => Ok(PddlFeature::Strips),
            ":typing" => Ok(PddlFeature::Typing),
            ":equality" => Ok(PddlFeature::Equality),
            ":negative-preconditions" => Ok(PddlFeature::NegativePreconditions),
            ":universal-preconditions" => Ok(PddlFeature::UniversalPreconditions),
            ":existential-preconditions" => Ok(PddlFeature::ExistentialPreconditions),
            ":quantified-preconditions" => Ok(PddlFeature::QuantifiedPreconditions),
            ":hierarchy" => Ok(PddlFeature::Hierarchy),
            ":method-preconditions" => Ok(PddlFeature::MethodPreconditions),
            ":durative-actions" => Ok(PddlFeature::DurativeAction),
            ":duration-inequalities" => Ok(PddlFeature::DurationInequalities),
            ":conditional-effects" => Ok(PddlFeature::ConditionalEffects),
            ":timed-initial-literals" => Ok(PddlFeature::TimedInitialLiterals),
            ":fluents" => Ok(PddlFeature::Fluents),
            ":numeric-fluents" => Ok(PddlFeature::NumericFluent),
            ":object-fluents" => Ok(PddlFeature::ObjectFluent),
            ":adl" => Ok(PddlFeature::Adl),
            ":preferences" => Ok(PddlFeature::Preferences),
            ":constraints" => Ok(PddlFeature::Constraints),
            ":action-costs" => Ok(PddlFeature::ActionCosts),
            ":goal-utilities" => Ok(PddlFeature::GoalUtilities),
            _ => Err(format!("Unknown feature `{s}`")),
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
            PddlFeature::UniversalPreconditions => ":universal-preconditions",
            PddlFeature::ExistentialPreconditions => ":existential-preconditions",
            PddlFeature::QuantifiedPreconditions => ":quantified-preconditions",
            PddlFeature::Hierarchy => ":hierarchy",
            PddlFeature::MethodPreconditions => ":method-preconditions",
            PddlFeature::DurativeAction => ":durative-action",
            PddlFeature::DurationInequalities => ":duration-inequalities",
            PddlFeature::ConditionalEffects => ":conditional-effects",
            PddlFeature::TimedInitialLiterals => ":timed-initial-literals",
            PddlFeature::Fluents => ":fluents",
            PddlFeature::NumericFluent => ":numeric-fluents",
            PddlFeature::ObjectFluent => ":object-fluents",
            PddlFeature::Adl => ":adl",
            PddlFeature::Preferences => ":preferences",
            PddlFeature::Constraints => ":constraints",
            PddlFeature::ActionCosts => ":action-costs",
            PddlFeature::GoalUtilities => ":goal-utilities",
        };
        write!(f, "{formatted}")
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
    pub constraints: Vec<SExpr>,
}
impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "# Domain : {}", self.name)?;
        write!(f, "\n# Types \n  ")?;
        disp_slice(f, self.types.as_slice(), "\n  ")?;
        write!(f, "\n# Predicates \n  ")?;
        disp_slice(f, self.predicates.as_slice(), "\n  ")?;
        write!(f, "\n# Functions \n  ")?;
        disp_slice(f, self.functions.as_slice(), "\n  ")?;
        write!(f, "\n# Tasks \n  ")?;
        disp_slice(f, self.tasks.as_slice(), "\n  ")?;
        write!(f, "\n# Methods \n  ")?;
        disp_slice(f, self.methods.as_slice(), "\n  ")?;
        write!(f, "\n# Actions \n  ")?;
        disp_slice(f, self.actions.as_slice(), "\n  ")?;
        write!(f, "\n# Durative Actions \n  ")?;
        disp_slice(f, self.durative_actions.as_slice(), "\n  ")?;
        write!(f, "\n# Constraints \n  ")?;
        disp_slice(f, self.constraints.as_slice(), "\n  ")?;

        Result::Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Tpe {
    pub name: Sym,
    pub parent: Sym,
}

pub type TypedSymbol = Param;

pub type Types = SmallVec<[Sym; 1]>;

/// Parameter to a function or action
#[derive(Debug, Clone)]
pub struct Param {
    /// name of the parameter
    pub symbol: Sym,
    /// Possible types of the parameter (any if empty)
    pub tpe: Types,
}
impl Param {
    pub fn new(symbol: impl Into<Sym>, tpe: impl Into<Sym>) -> Self {
        Self {
            symbol: symbol.into(),
            tpe: smallvec![tpe.into()],
        }
    }

    pub fn new_union(symbol: impl Into<Sym>, tpe: Types) -> Self {
        Self {
            symbol: symbol.into(),
            tpe,
        }
    }
}

impl Display for Param {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self.tpe.as_slice() {
            [tpe] => write!(f, "{}: {}", self.symbol, tpe),
            [] => write!(f, "{}", self.symbol),
            several => {
                write!(f, "{}: {{", self.symbol)?;
                disp_slice(f, several, ", ")?;
                write!(f, "}}")
            }
        }
    }
}

/// A PDDL predicate, i.e., state function whose codomain is the set of booleans.
#[derive(Debug, Clone)]
pub struct Predicate {
    pub name: Sym,
    pub args: Vec<Param>,
    pub source: Option<Span>,
}
impl Display for Predicate {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_slice(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

/// /// A PDDL function, i.e., state function whose codomain is the set of reals.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: Sym,
    pub args: Vec<Param>,
    pub tpe: Option<Sym>,
    pub source: Option<Span>,
}

impl Function {
    fn new(name: Sym, args: Vec<Param>, tpe: Option<Sym>, source: Option<Span>) -> Self {
        Self {
            name,
            args,
            tpe,
            source,
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name)?;
        disp_slice(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: Sym,
    pub args: Vec<TypedSymbol>,
    pub source: Option<Span>,
}

impl Display for TaskDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_slice(f, self.args.as_slice(), ", ")?;
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
    pub source: Option<Span>,
}
impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} ", self.name)?;
        disp_slice(f, &self.arguments, " ")?;
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
    pub source: Option<Span>,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone, Default, Debug)]
pub struct TaskNetwork {
    pub parameters: Vec<TypedSymbol>,
    pub ordered_tasks: Vec<Task>,
    pub unordered_tasks: Vec<Task>,
    pub orderings: Vec<Ordering>,
    pub constraints: Vec<SExpr>,
}

/// Constraint specifying that the task identified by `first_task_id` should end
/// before the one identified by `second_task_id`
#[derive(Clone, Debug)]
pub struct Ordering {
    pub first_task_id: TaskId,
    pub second_task_id: TaskId,
    source: Option<Span>,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub name: Sym,
    pub args: Vec<Param>,
    pub pre: Vec<SExpr>,
    pub eff: Vec<SExpr>,
    /// Span covering the entire action definition
    pub span: Span,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_slice(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Clone, Debug)]
pub struct DurativeAction {
    pub name: Sym,
    pub args: Vec<Param>,
    pub duration: SExpr,
    pub conditions: Vec<SExpr>,
    pub effects: Vec<SExpr>,
    /// Span covering the entire action definition
    pub span: Span,
}

impl Display for DurativeAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}(", self.name)?;
        disp_slice(f, self.args.as_slice(), ", ")?;
        write!(f, ")")
    }
}

#[derive(Debug)]
pub enum Plan {
    ActionSequence(Vec<ActionInstance>),
}

#[derive(Debug)]
pub struct ActionInstance {
    pub name: Sym,
    pub arguments: Vec<Sym>,
    pub span: Span,
}

impl Spanned for ActionInstance {
    fn span(&self) -> Option<&Span> {
        Some(&self.span)
    }
}
impl Display for ActionInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} {})", self.name, self.arguments.iter().format(" "))
    }
}

/// Consume a typed list of symbols
///  - (a - loc b - loc c - loc) : symbols a, b and c of type loc
///  - (a b c - loc)  : symbols a, b and c of type loc
///  - (a b c) : symbols a b and c of type object
pub fn consume_typed_symbols(input: &mut ListIter) -> std::result::Result<Vec<TypedSymbol>, Message> {
    let mut args = Vec::with_capacity(input.len() / 3);
    let mut untyped: Vec<Sym> = Vec::with_capacity(args.len());
    while !input.is_empty() {
        let next = input.pop_atom()?;
        if next.canonical_str() == "-" {
            let mut types = Types::with_capacity(1);
            let tpe = input.pop()?;
            if let Some(variants) = tpe.as_application("either") {
                for variant in variants {
                    types.push(
                        variant
                            .as_atom()
                            .cloned()
                            .ok_or(variant.invalid("expected type name"))?,
                    );
                }
            } else {
                types.push(tpe.as_atom().cloned().ok_or(tpe.invalid("expected type name"))?);
            }
            untyped
                .drain(..)
                .map(|name| TypedSymbol::new_union(name, types.clone()))
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
            tpe: smallvec![],
        })
        .for_each(|a| args.push(a));
    Result::Ok(args)
}

/// Returns a localized error on `expr` if the given feature is not present in the domain.
fn check_feature_presence(feature: PddlFeature, domain: &Domain, expr: &SExpr) -> Result<(), Message> {
    if domain.features.contains(&feature) {
        Ok(())
    } else {
        Err(expr.invalid(format!("Requires the {feature} feature in the requirements.")))
    }
}

fn read_domain(dom: SExpr) -> std::result::Result<Domain, Message> {
    let dom = &mut dom.as_list_iter().ok_or_else(|| dom.invalid("Expected a list"))?;

    dom.pop_known_atom("define")?;

    // extract the name of the domain, of the form `(domain XXX)`
    let mut domain_name_decl = dom.pop_list()?.iter();
    domain_name_decl.pop_known_atom("domain")?;
    let name = domain_name_decl.pop_atom().title("missing name of domain")?.clone();

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
        constraints: vec![],
    };

    for current in dom {
        // a property associates a key (e.g. `:predicates`) to a value or a sequence of values
        let mut property = current
            .as_list_iter()
            .ok_or_else(|| current.invalid("expected a property list"))?;

        match property.pop_atom()?.canonical_str() {
            ":requirements" => {
                for feature in property {
                    let feature = feature
                        .as_atom()
                        .ok_or_else(|| feature.invalid("Expected feature name but got list"))?;
                    let f = PddlFeature::from_str(feature.canonical_str()).map_err(|e| feature.invalid(e))?;

                    res.features.push(f);
                }
            }
            ":predicates" => {
                for pred in property {
                    let mut pred = pred.as_list_iter().ok_or_else(|| pred.invalid("Expected a list"))?;
                    let name = pred.pop_atom()?.clone();
                    let args = consume_typed_symbols(&mut pred)?;
                    res.predicates.push(Predicate {
                        name,
                        args,
                        source: Some(pred.loc()),
                    });
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
                while let Ok(func) = property.pop() {
                    // element is necessarily a function name and parameters, e.g., (battery ?r)
                    let mut func = func.as_list_iter().ok_or_else(|| func.invalid("Expected a list"))?;
                    let name = func.pop_atom()?.clone();
                    let args = consume_typed_symbols(&mut func)?;

                    // from PDDL 3.1, it can have a type annotation, e.g., (battery ?r) - number
                    // whici allows distinguishing numeric and object fluents
                    let tpe = if property.peek().is_some_and(|a| a.is_atom("-")) {
                        property.pop_known_atom("-")?;
                        Some(property.pop_atom().title("expected a type").cloned()?)
                    } else {
                        None
                    };
                    res.functions.push(Function::new(name, args, tpe, Some(func.loc())));
                }
            }
            ":action" => {
                let name = property.pop_atom()?.clone();
                let mut args = Vec::new();
                let mut pre = Vec::new();
                let mut eff = Vec::new();
                while !property.is_empty() {
                    let key_expr = property.pop_atom()?;
                    let value = property.pop().tag(key_expr, "No value associated to arg", None)?;
                    match key_expr.canonical_str() {
                        ":parameters" => {
                            if !args.is_empty() {
                                return Err(key_expr.invalid("Duplicated ':parameters' tag is not allowed"));
                            }
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
                        _ => return Err(key_expr.invalid("unsupported key in action")),
                    }
                }
                res.actions.push(Action {
                    name,
                    args,
                    pre,
                    eff,
                    span: current.loc(),
                })
            }
            ":durative-action" => {
                let name = property.pop_atom()?.clone();
                let mut args = Vec::new();
                let mut duration = None;
                let mut conditions = Vec::new();
                let mut effects = Vec::new();

                while let Ok(key_expr) = property.pop_atom() {
                    let key = key_expr.to_string();
                    let value = property.pop().title(format!("No value associated to arg: {key}"))?; // TODO
                    match key.as_str() {
                        ":parameters" => {
                            if !args.is_empty() {
                                return Err(key_expr.invalid("Duplicated ':parameters' tag is not allowed"));
                            }
                            let mut value = value
                                .as_list_iter()
                                .ok_or_else(|| value.invalid("Expected a parameter list"))?;
                            for a in consume_typed_symbols(&mut value)? {
                                args.push(a);
                            }
                        }
                        ":duration" => {
                            if duration.is_some() {
                                return Err(key_expr.invalid("Duration was previously set."));
                            }
                            duration = Some(value.clone());
                        }
                        ":condition" => {
                            conditions.push(value.clone());
                        }
                        ":effect" => {
                            effects.push(value.clone());
                        }
                        _ => return Err(key_expr.invalid("unsupported key in action")),
                    }
                }
                let duration = duration.ok_or_else(|| current.invalid("Action has no duration field"))?;
                let durative_action = DurativeAction {
                    name,
                    args,
                    duration,
                    conditions,
                    effects,
                    span: current.loc(),
                };
                res.durative_actions.push(durative_action)
            }
            ":task" => {
                check_feature_presence(PddlFeature::Hierarchy, &res, current)?;
                let name = property.pop_atom().title("Missing task name")?.clone();
                property.pop_known_atom(":parameters")?;
                let params = property.pop_list().title("Expected a parameter list")?;
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
                let name = property.pop_atom().title("Missing task name")?.clone();
                property.pop_known_atom(":parameters")?;
                let params = property.pop_list().title("Expected a parameter list")?;
                let parameters = consume_typed_symbols(&mut params.iter())?;
                property.pop_known_atom(":task")?;
                let task = parse_task(property.pop()?, false)?;
                let precondition = if property.peek().is_some_and(|e| e.is_atom(":precondition")) {
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
            ":constraints" => {
                for constraint in property {
                    res.constraints.push(constraint.clone());
                }
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
        match key.canonical_str() {
            ":ordered-tasks" | ":ordered-subtasks" => {
                if !tn.ordered_tasks.is_empty() {
                    return Err(key.invalid("More than one set of ordered tasks."));
                }
                let value = key_values.pop()?;
                tn.ordered_tasks = parse_conjunction(value, |e| parse_task(e, true))?;
            }
            ":tasks" | ":subtasks" => {
                if !tn.unordered_tasks.is_empty() {
                    return Err(key.invalid("More than one set of unordered tasks."));
                }
                let value = key_values.pop()?;
                tn.unordered_tasks = parse_conjunction(value, |e| parse_task(e, true))?;
            }
            ":ordering" => {
                if !tn.orderings.is_empty() {
                    return Err(key.invalid("More than one set of ordering constraints."));
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
                let value = key_values.pop()?;
                let mut value = value
                    .as_list_iter()
                    .ok_or_else(|| value.invalid("Expected a parameter list"))?;
                for a in consume_typed_symbols(&mut value)? {
                    tn.parameters.push(a);
                }
            }
            ":constraints" => {
                let value = key_values.pop()?;
                tn.constraints.push(value.clone());
            }
            _ => return Err(key.invalid("Unsupported keyword in task network")),
        }
    }
    Ok(tn)
}

fn parse_task(e: &SExpr, allow_id: bool) -> std::result::Result<Task, Message> {
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

type R<T> = std::result::Result<T, Message>;

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
    pub metric: Option<Metric>,
    pub constraints: Vec<SExpr>,
}

#[derive(Clone, Debug)]
pub enum Metric {
    Minimize(SExpr),
    Maximize(SExpr),
}

impl Display for Problem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "# Problem {} (domain: {})", &self.problem_name, &self.domain_name)?;
        write!(f, "\n# Objects \n  ")?;
        disp_slice(f, self.objects.as_slice(), "\n  ")?;
        write!(f, "\n# Init \n  ")?;
        disp_slice(f, self.init.as_slice(), "\n  ")?;
        write!(f, "\n# Goal \n  ")?;
        disp_slice(f, self.goal.as_slice(), "\n  ")?;
        if let Some(tn) = &self.task_network {
            write!(f, "\n# Tasks \n")?;
            for task in tn.ordered_tasks.iter().chain(tn.unordered_tasks.iter()) {
                writeln!(f, "  {task}")?;
            }
        }
        Result::Ok(())
    }
}

fn read_problem(problem: SExpr) -> std::result::Result<Problem, Message> {
    let mut problem = problem
        .as_list_iter()
        .ok_or_else(|| problem.invalid("Expected a list"))?;
    problem.pop_known_atom("define")?;

    let mut problem_name = problem
        .pop_list()
        .title("Expected problem name definition of the form '(problem XXXXXX)'")?
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
        metric: None,
        constraints: vec![],
    };

    for current in problem {
        // a property associates a key (e.g. `:objects`) to a value or a sequence of values
        let mut property = current
            .as_list_iter()
            .ok_or_else(|| current.invalid("Expected a list"))?;
        match property.pop_atom()?.canonical_str() {
            ":requirements" => {} // HACK: ignore requirements in problem (umtranslog, IPC 2002)
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
            ":metric" => {
                let qualifier = property.pop_atom()?;
                match qualifier.canonical_str() {
                    "minimize" => res.metric = Some(Metric::Minimize(property.pop().cloned()?)),
                    "maximize" => res.metric = Some(Metric::Maximize(property.pop().cloned()?)),
                    _ => return Err(qualifier.invalid("expected `maximize` or `minimize")),
                }
            }
            ":constraints" => {
                for constraint in property {
                    res.constraints.push(constraint.clone());
                }
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

    fn parse(s: &str) -> Res<SExpr> {
        let s = Input::from_string(s);
        super::parse(Arc::new(s))
    }

    #[test]
    fn parsing() -> Result<(), String> {
        let prog = "(begin (define r 10) (* pi (* r r)))";
        match parse(prog) {
            Result::Ok(e) => println!("{e}"),
            Result::Err(s) => eprintln!("{s}"),
        }

        Result::Ok(())
    }

    #[test]
    fn parsing_hddl() -> Res<()> {
        let source = "../problems/hddl/tests/nothing.dom.hddl";
        let source = PathBuf::from_str(source)?;
        let source = Arc::new(Input::from_file(&source)?);

        match super::parse(source) {
            Result::Ok(e) => {
                println!("{e}");

                let dom = match read_domain(e) {
                    Ok(dom) => dom,
                    Err(e) => {
                        eprintln!("{:?}", &e);
                        panic!("Could not parse")
                    }
                };

                println!("{dom}");
            }
            Result::Err(s) => eprintln!("{s}"),
        }

        Result::Ok(())
    }

    #[test]
    fn parse_gripper_plan() -> Res<()> {
        let source = "../problems/pddl/tests/gripper.plan";
        let source = PathBuf::from_str(source)?;
        let source = Input::from_file(&source)?;
        super::parse_plan(source)?;
        Ok(())
    }
}
