use std::rc::Rc;

use errors::*;
use itertools::Itertools;
use pddl::sexpr::SExpr;

use crate::pddl::sexpr::ListIter;
use crate::*;

use super::parser::{Domain, Problem};

pub struct Bindings {
    lasts: hashbrown::HashMap<crate::Sym, Expr>,
    prev: Option<Rc<Bindings>>,
}

impl Bindings {
    pub fn objects(objects: &Objects) -> Self {
        let mut lasts = hashbrown::HashMap::new();
        for o in objects.iter() {
            lasts.insert(o.name().clone(), Expr::Object(o.clone()));
        }
        Self { lasts, prev: None }
    }

    pub fn stacked(params: &[Param], prev: &Rc<Bindings>) -> Self {
        let mut lasts = hashbrown::HashMap::new();
        for p in params {
            lasts.insert(p.name().clone(), Expr::Param(p.clone()));
        }
        Self {
            lasts,
            prev: Some(prev.clone()),
        }
    }

    pub fn get(&self, name: impl Into<crate::Sym>) -> Result<Expr, Message> {
        let name = name.into();
        if let Some(expr) = self.lasts.get(&name) {
            Ok(expr.clone())
        } else if let Some(prev) = self.prev.as_ref() {
            prev.get(name)
        } else {
            Err(Message::error("Unknown symbol").snippet(name.error("unrecognized")))
        }
    }
}

fn user_types(dom: &Domain) -> Result<UserTypes, Message> {
    let mut types = UserTypes::new();
    for tpe in &dom.types {
        match tpe.tpe.as_slice() {
            [] => types.add_type(&tpe.symbol, None),
            [parent] => types.add_type(&tpe.symbol, Some(parent)),
            [_, second_parent, ..] => {
                return Err(second_parent
                    .invalid("unexpected second parent type")
                    .info(&tpe.symbol, "for type"));
            }
        }
    }
    Ok(types)
}

pub fn build_model(dom: &Domain, prob: &Problem) -> Res<Model> {
    // top types in pddl

    let types = user_types(dom)?;
    let types = Types::new(types);
    let mut model = Model::new(types);

    for pred in &dom.predicates {
        let parameters = parse_parameters(&pred.args, &model.env.types).msg(&model.env)?;
        model
            .env
            .fluents
            .add_fluent(&pred.name, parameters, Type::Bool, pred.source.clone())?;
    }

    for func in &dom.functions {
        let parameters = parse_parameters(&func.args, &model.env.types).msg(&model.env)?;
        let tpe = match func.tpe.as_ref().map(|t| t.canonical_str()) {
            None | Some("number") => Type::REAL,
            Some(name) => {
                let user_type = model.env.types.get_user_type(name).msg(&model.env)?;
                user_type.into()
            }
        };
        model
            .env
            .fluents
            .add_fluent(&func.name, parameters, tpe, func.source.clone())
            .msg(&model.env)?;
    }

    for obj in dom.constants.iter().chain(prob.objects.iter()) {
        let tpe = match obj.tpe.as_slice() {
            [] => model.env.types.top_user_type(),
            [tpe] => model.env.types.get_user_type(tpe).msg(&model.env)?,
            [_, tpe, ..] => return Err(tpe.invalid("object with more than one type")),
        };
        model.env.objects.add_object(&obj.symbol, tpe).msg(&model.env)?;
    }

    let bindings = Rc::new(Bindings::objects(&model.env.objects));

    for init in &prob.init {
        let effs = into_effects(Some(Timestamp::ORIGIN), init, &mut model.env, &bindings)?;
        model.init.extend(effs);
    }

    for g in &prob.goal {
        let sub_goals = conjuncts(g);
        for g in sub_goals {
            if is_preference(g) {
                let pref = parse_goal_preference(g, true, &mut model.env, &bindings)?;
                model.preferences.add(pref);
            } else {
                let g = parse_goal(g, true, &mut model.env, &bindings)?;
                model.goals.push(g);
            }
        }
    }

    let all_constraints = dom.constraints.iter().chain(prob.constraints.iter());
    for c in all_constraints {
        let sub_goals = conjuncts(c);
        for c in sub_goals {
            if is_preference(c) {
                let pref = parse_goal_preference(c, false, &mut model.env, &bindings)?;
                model.preferences.add(pref);
            } else {
                let c = parse_goal(c, false, &mut model.env, &bindings)?;
                model.goals.push(c);
            }
        }
    }
    if let Some(metric) = &prob.metric {
        match metric {
            pddl::Metric::Maximize(max) => {
                model.metric = Some(Metric::Maximize(parse(max, &mut model.env, &bindings)?))
            }
            pddl::Metric::Minimize(min) => {
                model.metric = Some(Metric::Minimize(parse(min, &mut model.env, &bindings)?))
            }
        }
    }

    for t in &dom.tasks {
        let params = parse_parameters(&t.args, &model.env.types).msg(&model.env)?;
        model.actions.add_task(t.name.clone(), params)?;
    }

    for a in &dom.actions {
        parse_action(a, &mut model.env, &bindings)
            .and_then(|action| model.actions.add(action, &model.env))
            .tag(&a.name, "when parsing action", Some(&a.span))?;
    }

    for a in &dom.durative_actions {
        parse_durative_action(a, &mut model.env, &bindings)
            .and_then(|action| model.actions.add(action, &model.env))
            .tag(&a.name, "when parsing action", Some(&a.span))?;
    }

    for m in &dom.methods {
        parse_method(m, &mut model.env, &bindings, &model.actions)
            .and_then(|action| model.actions.add(action, &model.env))
            .tag(&m.name, "when parsing method", m.source.as_ref())?;
    }

    if let Some(tn) = &prob.task_network {
        let tn = parse_task_net(tn, &model.actions, &mut model.env, &bindings)?;
        model.task_network = Some(tn);
    }

    Ok(model)
}

pub fn build_plan(plan: &pddl::Plan, model: &Model) -> Res<Plan> {
    match plan {
        pddl::Plan::ActionSequence(action_instances) => {
            let mut operators = Vec::with_capacity(action_instances.len());
            for a in action_instances {
                let action = model
                    .actions
                    .get_action(&a.name)
                    .ok_or_else(|| a.name.invalid("Unknown action"))?;
                if a.arguments.len() != action.parameters.len() {
                    return Err(a.invalid(format!(
                        "Wrong number of parameters. Expected {}, provided: {}",
                        action.parameters.len(),
                        a.arguments.len()
                    )));
                }
                let mut arguments = Vec::with_capacity(a.arguments.len());
                for (arg, param) in a.arguments.iter().zip(action.parameters.iter()) {
                    let obj = model.env.objects.get(arg).msg(&model.env)?;
                    if !Type::from(obj.tpe()).is_subtype_of(param.tpe()) {
                        return Err(arg.invalid(format!(
                            "Object has type `{} ` that is incompatible with the expected type for parameter`{}`",
                            obj.tpe(),
                            param
                        )));
                    }
                    arguments.push(obj);
                }
                operators.push(Operator {
                    action_ref: action.name.clone(),
                    arguments,
                    span: Some(a.span.clone()),
                });
            }
            Ok(Plan::Sequential(operators))
        }
    }
}

fn parse_action(a: &pddl::Action, env: &mut Environment, bindings: &Rc<Bindings>) -> Result<Action, Message> {
    let parameters = parse_parameters(&a.args, &env.types).msg(env)?;

    let bindings = Rc::new(Bindings::stacked(&parameters, bindings));

    let mut action = Action::instantaneous(&a.name, parameters, env)?;

    let action_start = action.start();
    let condition_parser: &ExprParser<Condition> = &move |c, env, bindings| {
        let c = parse(c, env, bindings)?;
        Ok(Condition::at(action_start, c))
    };
    for c in &a.pre {
        for c in conjuncts(c) {
            if is_preference(c) {
                let pref = parse_preference_gen(c, condition_parser, env, &bindings)?;
                action.preferences.add(pref);
            } else {
                let cond = condition_parser(c, env, &bindings)?;
                action.conditions.push(cond);
            }
        }
    }

    for e in &a.eff {
        let effs = into_effects(Some(action.end().into()), e, env, &bindings)?;
        action.effects.extend_from_slice(&effs);
    }
    Ok(action)
}

fn parse_durative_action(
    a: &pddl::DurativeAction,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Action, Message> {
    let parameters = parse_parameters(&a.args, &env.types).msg(env)?;

    let bindings = Rc::new(Bindings::stacked(&parameters, bindings));
    let duration = parse_duration(&a.duration, env, &bindings)?;

    let mut action = Action::new(&a.name, parameters, duration, env)?;

    let condition_parser: &ExprParser<Condition> = &move |c, env, bindings| {
        let (itv, c) = parse_timed(c, env, bindings)?;
        Ok(Condition::over(itv, c))
    };
    for c in &a.conditions {
        for c in conjuncts(c) {
            if is_preference(c) {
                let pref = parse_preference_gen(c, condition_parser, env, &bindings)?;
                action.preferences.add(pref);
            } else {
                let cond = condition_parser(c, env, &bindings)?;
                action.conditions.push(cond);
            }
        }
    }

    for e in &a.effects {
        let effs = into_effects(None, e, env, &bindings)?;
        action.effects.extend(effs);
    }
    Ok(action)
}

fn parse_method(
    a: &pddl::Method,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
    actions: &Actions,
) -> Result<Action, Message> {
    let parameters = parse_parameters(&a.parameters, &env.types).msg(env)?;

    let bindings = Rc::new(Bindings::stacked(&parameters, bindings));

    let achieved = {
        let name = a.task.name.clone();
        let args: Vec<ExprId> = a
            .task
            .arguments
            .iter()
            .cloned()
            .map(|a| parse(&SExpr::Atom(a), env, &bindings))
            .try_collect()?;
        AchievedTask { name, args }
    };

    let mut action = Action::method(a.name.clone(), parameters, achieved);

    let action_start = action.start();
    let condition_parser: &ExprParser<Condition> = &move |c, env, bindings| {
        let c = parse(c, env, bindings)?;
        Ok(Condition::at(action_start, c))
    };
    for c in &a.precondition {
        for c in conjuncts(c) {
            if is_preference(c) {
                let pref = parse_preference_gen(c, condition_parser, env, &bindings)?;
                action.preferences.add(pref);
            } else {
                let cond = condition_parser(c, env, &bindings)?;
                action.conditions.push(cond);
            }
        }
    }

    action.subtasks = parse_task_net(&a.subtask_network, actions, env, &bindings)?;

    Ok(action)
}

fn parse_task_net(
    task_net: &pddl::TaskNetwork,
    actions: &Actions,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Res<TaskNet> {
    /// helper function that creates a precedence expression (< first second)
    fn prec(first: SubtaskId, second: SubtaskId, env: &mut Environment) -> Res<ExprId> {
        let end_first = env.intern(Expr::Instant(first.end().into()), None)?;
        let start_second = env.intern(Expr::Instant(second.start().into()), None)?;
        let precedes = env.intern(Expr::App(Fun::Lt, smallvec::smallvec![end_first, start_second]), None)?;
        Ok(precedes)
    }

    let mut tn = TaskNet::default();

    // extract variables in the task ntework, they will be add to the bindings of the current scope
    // to allow their usage to be recognized
    let params = parse_parameters(&task_net.parameters, &env.types).msg(env)?;
    let bindings = &Rc::new(Bindings::stacked(&params, bindings));
    tn.variables = params;

    for s in &task_net.unordered_tasks {
        let subtask = parse_subtask(s, env, actions, bindings)?;
        tn.add(subtask, env)?;
    }
    {
        let mut last_task_id: Option<SubtaskId> = None;
        for s in &task_net.ordered_tasks {
            let subtask = parse_subtask(s, env, actions, bindings)?;
            let id = tn.add(subtask, env)?;
            if let Some(prev) = last_task_id {
                // add precedence constraint with the previously parsed task
                tn.constraints.push(prec(prev, id, env)?);
            }

            last_task_id = Some(id);
        }
    }

    for ord in &task_net.orderings {
        let first = tn.get_id_by_ref(&ord.first_task_id)?;
        let second = tn.get_id_by_ref(&ord.second_task_id)?;
        tn.constraints.push(prec(first, second, env)?);
    }

    Ok(tn)
}

fn into_effects(
    default_timestamp: Option<Timestamp>,
    expr: &SExpr,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Vec<Effect>, Message> {
    let mut all_effs = Vec::new();

    for expr in conjuncts(expr) {
        if let Some([vars, quantified_expr]) = expr.as_application("forall") {
            let vars = parse_var_list(vars, env)?;
            // stack bindings so that downstream expression know the declared variables
            let bindings = Rc::new(Bindings::stacked(&vars, bindings));
            let effs = into_effects(default_timestamp, quantified_expr, env, &bindings)?;
            all_effs.extend(effs.into_iter().map(|e| e.with_quantification(&vars)))
        } else if let Some([tp, expr]) = expr.as_application("at")
            && let Ok(time) = parse_timestamp(tp)
        {
            // (at end (not (loc r1 l1)))   or   (at 12.3 (loc r1 l2))
            // in the condition we check that we indeed have a valid timepoint because it is common to have also an `at` fluent
            let effs = into_effects(Some(time), expr, env, bindings)?;
            all_effs.extend(effs);
        } else if let Some([cond, expr]) = expr.as_application("when") {
            let cond = parse(cond, env, bindings)?;
            let effs = into_effects(default_timestamp, expr, env, bindings)?;
            for eff in effs {
                if let Some(other_cond) = eff.effect_expression.condition {
                    return Err(env.node(other_cond).invalid("expected second condition"));
                }
                all_effs.push(eff.with_condition(cond));
            }
        } else {
            let Some(time) = default_timestamp else {
                return Err(expr.invalid("expected temporal qualifier, e.g., (at end ...)"));
            };
            let eff = into_effect(time, expr, env, bindings)?;
            all_effs.push(eff.not_quantified());
        }
    }

    Ok(all_effs)
}

fn into_effect(
    time: impl Into<Timestamp>,
    expr: &SExpr,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<SimpleEffect, Message> {
    fn parse_sv(
        expr: &SExpr,
        expected_type: Option<Type>,
        env: &mut Environment,
        bindings: &Rc<Bindings>,
    ) -> Result<StateVariable, Message> {
        let x = parse(expr, env, bindings)?;
        if let Some(expected_type) = expected_type {
            expected_type.accepts(x, env).msg(env)?;
        }
        let (fluent, sv_args) = env.node(x).state_variable()?;
        Ok(StateVariable::new(
            fluent,
            sv_args.iter().copied().collect(),
            expr.loc(),
        ))
    }
    if let Some([arg]) = expr.as_application("not") {
        let contradiction = env.intern(Expr::Bool(false), None)?;
        let sv = parse_sv(arg, Some(Type::Bool), env, bindings)?;
        Ok(SimpleEffect::assignement(time.into(), sv, contradiction))
    } else if let Some([sv, val]) = expr.as_application("=") {
        let sv = parse_sv(sv, None, env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(SimpleEffect::assignement(time.into(), sv, val))
    } else if let Some([sv, val]) = expr.as_application("assign") {
        let sv = parse_sv(sv, None, env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(SimpleEffect::assignement(time.into(), sv, val))
    } else if let Some([sv, val]) = expr.as_application("increase") {
        // (increase (fuel-level r1) 2)
        let sv = parse_sv(sv, Some(Type::Real), env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(SimpleEffect::increase(time.into(), sv, val))
    } else if let Some([sv, val]) = expr.as_application("decrease") {
        // (increase (fuel-level r1) 2)
        let sv = parse_sv(sv, Some(Type::Real), env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(SimpleEffect::decrease(time.into(), sv, val))
    } else {
        let tautology = env.intern(Expr::Bool(true), None)?;
        let sv = parse_sv(expr, Some(Type::Bool), env, bindings)?;
        Ok(SimpleEffect::assignement(time.into(), sv, tautology))
    }
}

fn parse_subtask(t: &pddl::Task, env: &mut Environment, actions: &Actions, bindings: &Rc<Bindings>) -> Res<Subtask> {
    let name = &t.name;
    if let Some(declared_task) = actions.get_task(name) {
        let args: Vec<ExprId> = t
            .arguments
            .iter()
            .map(|p| parse(&SExpr::Atom(p.clone()), env, bindings))
            .try_collect()?;
        declared_task.check_application(name, &args, env)?;
        Ok(Subtask {
            ref_name: t.id.clone(),
            task_name: name.clone(),
            args,
            source: t.source.clone(),
        })
    } else {
        Err(name.invalid("expected a task name"))
    }
}

fn parse_parameters(params: &[pddl::Param], types: &Types) -> Result<Vec<Param>, TypeError> {
    let mut parameters = Vec::with_capacity(params.len());
    for a in params {
        let tpe = types.get_union_type(&a.tpe)?;
        parameters.push(Param::new(&a.symbol, tpe))
    }
    Ok(parameters)
}

/// Parses a list of variables for forall/exists : (?d - depot ?x - loc)
fn parse_var_list(vars: &SExpr, env: &Environment) -> Result<Vec<Param>, Message> {
    let mut vars = vars
        .as_list_iter()
        .ok_or_else(|| vars.invalid("expected variable list"))?;
    let vars = pddl::consume_typed_symbols(&mut vars)?;
    parse_parameters(&vars, &env.types).msg(env)
}

fn is_empty_list(sexpr: &SExpr) -> bool {
    match sexpr {
        SExpr::Atom(_) => false,
        SExpr::List(l) => l.iter().next().is_none(),
    }
}

/// Parses a conjunctions in to the set of
fn conjuncts(sexpr: &SExpr) -> Vec<&SExpr> {
    if is_empty_list(sexpr) {
        Vec::new()
    } else {
        match sexpr {
            SExpr::Atom(_) => vec![sexpr],
            SExpr::List(slist) => {
                let mut slist = slist.iter();
                if slist.pop_known_atom("and").is_ok() {
                    slist.collect_vec()
                } else {
                    vec![sexpr]
                }
            }
        }
    }
}

fn parse(sexpr: &SExpr, env: &mut Environment, bindings: &Rc<Bindings>) -> Result<ExprId, Message> {
    fn parse_args(l: ListIter<'_>, env: &mut Environment, bindings: &Rc<Bindings>) -> Result<SeqExprId, Message> {
        let mut args = SeqExprId::new();
        for e in l {
            let arg = parse(e, env, bindings)?;
            args.push(arg);
        }
        Ok(args)
    }

    let expr = match sexpr {
        SExpr::Atom(atom) if atom.canonical_str() == "?duration" => Expr::Duration,
        SExpr::Atom(atom) => match bindings.get(atom) {
            Ok(x) => x,
            Err(err) => {
                if let Some(number) = parse_number(atom.canonical_str()) {
                    Expr::Real(number)
                } else if let Some(fluent_id) = env.fluents.get_by_name(atom.canonical_str()) {
                    // this occurs case when a state variable is called without a parameter list
                    // for instance   (increase total-fuel 1)
                    // we emit the state variable without checking the args as any error should be caught when type checking anyway
                    Expr::StateVariable(fluent_id, SeqExprId::new())
                } else {
                    return Err(err);
                }
            }
        },
        SExpr::List(l) => {
            let mut l = l.iter();
            let f = l.pop_atom()?.clone();
            if let Some(f) = env.fluents.get_by_name(f.canonical_str()) {
                let args = parse_args(l, env, bindings)?;
                Expr::StateVariable(f, args)
            } else if let Some(f) = parse_function(&f) {
                let args = parse_args(l, env, bindings)?;
                Expr::App(f, args)
            } else if f.canonical_str() == "total-time" {
                if let Some(x) = l.next() {
                    return Err(x.invalid("Unexpected argument to total-time"));
                }
                Expr::Makespan
            } else if f.canonical_str() == "is-violated" {
                let id = l.pop_atom()?;
                Expr::ViolationCount(id.into())
            } else if f.canonical_str() == "exists" || f.canonical_str() == "forall" {
                let vars = parse_var_list(l.pop()?, env)?;
                let expr = l.pop()?;
                let bindings = Rc::new(Bindings::stacked(&vars, bindings));
                let expr = parse(expr, env, &bindings)?; // TODO
                match f.canonical_str() {
                    "forall" => Expr::Forall(vars, expr),
                    "exists" => Expr::Exists(vars, expr),
                    _ => unreachable!(),
                }
            } else {
                return Err(f.invalid("unknown atom"));
            }
        }
    };
    env.intern(expr, sexpr.loc()).map_err(|e| {
        e.snippet(
            sexpr
                .loc()
                .annotate(annotate_snippets::Level::INFO, "when parsing expression"), // TODO: simplify
        )
    })
}

/// Parses a goal or constraint, possibly with a forall quanifiier
pub fn parse_goal(
    sexpr: &SExpr,
    at_horizon: bool,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Goal, Message> {
    if let Some([vars, sexpr]) = sexpr.as_application("forall")
        && !at_horizon
    {
        // (forall (?x - loc ?y - obj) <constraint>)
        let vars = parse_var_list(vars, env)?;
        let bindings = Rc::new(Bindings::stacked(&vars, bindings));
        parse_unquantified_goal(sexpr, at_horizon, env, &bindings).map(|g| g.forall(vars))
    } else {
        parse_unquantified_goal(sexpr, at_horizon, env, bindings).map(|g| g.forall(vec![]))
    }
}

/// Parses a goal (at_horizon=true) of constraint (at_horizon=false), without a forall
pub fn parse_unquantified_goal(
    sexpr: &SExpr,
    at_horizon: bool,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<SimpleGoal, Message> {
    if at_horizon {
        let goal = parse(sexpr, env, bindings)?;
        Ok(SimpleGoal::at(Timestamp::HORIZON, goal))
    } else if let Some([tp, g]) = sexpr.as_application("at") {
        if !tp.is_atom("end") {
            return Err(tp.invalid("expected `end`"));
        }
        let g = parse(g, env, bindings)?;
        Ok(SimpleGoal::at(Timestamp::HORIZON, g))
    } else if let Some([tp, g]) = sexpr.as_application("within") {
        let tp = tp
            .as_atom()
            .and_then(|n| parse_number(n.canonical_str()))
            .ok_or(tp.invalid("expected number"))?;
        let g = parse(g, env, bindings)?;
        Ok(SimpleGoal::at(tp, g))
    } else if let Some([g]) = sexpr.as_application("always") {
        let g = parse(g, env, bindings)?;
        Ok(SimpleGoal::HoldsDuring(TimeInterval::FULL, g))
    } else if let Some([g]) = sexpr.as_application("sometime") {
        let g = parse(g, env, bindings)?;
        Ok(SimpleGoal::SometimeDuring(TimeInterval::FULL, g))
    } else if let Some([g]) = sexpr.as_application("at-most-once") {
        let g = parse(g, env, bindings)?;
        Ok(SimpleGoal::AtMostOnceDuring(TimeInterval::FULL, g))
    } else if let Some([when, then]) = sexpr.as_application("sometime-before") {
        let when = parse(when, env, bindings)?;
        let then = parse(then, env, bindings)?;
        Ok(SimpleGoal::SometimeBefore { when, then })
    } else if let Some([when, then]) = sexpr.as_application("sometime-after") {
        let when = parse(when, env, bindings)?;
        let then = parse(then, env, bindings)?;
        Ok(SimpleGoal::SometimeAfter { when, then })
    } else if let Some([delay, when, then]) = sexpr.as_application("always-within") {
        let delay = parse_number_sexpr(delay)?;
        let when = parse(when, env, bindings)?;
        let then = parse(then, env, bindings)?;
        Ok(SimpleGoal::AlwaysWithin { delay, when, then })
    } else {
        Err(sexpr.invalid("invalid goal expression"))
    }
}

fn is_preference(sexpr: &SExpr) -> bool {
    if let Some([_params, expr]) = sexpr.as_application("forall") {
        is_preference(expr)
    } else {
        sexpr.as_application("preference").is_some()
    }
}

/// Parses a preference:
///  - (preference <name> <goal-expr>)
///  - (forall (?a - b ?x - t) preference <name> <goal-expr>)
///
/// If `at_horizon` is true, the goal expression is supposed to hold at the horizon,
/// otherwise it is a PDDL constraint (within, always, ....)
fn parse_goal_preference(
    sexpr: &SExpr,
    at_horizon: bool,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Preference<Goal>, Message> {
    parse_preference_gen(
        sexpr,
        &move |e, env, bindings| parse_goal(e, at_horizon, env, bindings),
        env,
        bindings,
    )
}

type ExprParser<T> = dyn Fn(&SExpr, &mut Environment, &Rc<Bindings>) -> Res<T>;

/// Parses a preference of the form
///   (forall (?x - loc ?y -obj) (preference pref-name <T>))
///   (preference pref-name <T>)
/// Given a parser for T
fn parse_preference_gen<T>(
    sexpr: &SExpr,
    sub_parser: &ExprParser<T>,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Preference<T>, Message> {
    if let Some([id, goal]) = sexpr.as_application("preference") {
        let id = id
            .as_atom()
            .ok_or(id.invalid("expected preference identifier"))
            .cloned()?;
        let goal = sub_parser(goal, env, bindings)?;
        Ok(Preference::new(id, goal))
    } else if let Some([vars, pref]) = sexpr.as_application("forall") {
        // we have a forall, first identify the variables and add them to the bindings
        let vars = parse_var_list(vars, env)?;
        let bindings = Rc::new(Bindings::stacked(&vars, bindings));
        // parse the preference (with the additional bindings) and then append the variables
        parse_preference_gen(pref, sub_parser, env, &bindings).map(|pref| pref.forall(vars))
    } else {
        Err(sexpr.invalid("malformed preference"))
    }
}

/// Parse a number number ("32", "-3", "3.14", -323.3")
fn parse_number(decimal_str: &str) -> Option<RealValue> {
    if let Ok(i) = decimal_str.parse::<IntValue>() {
        Some(RealValue::new(i, 1))
    } else {
        let (lhs, rhs) = decimal_str.split_once(".")?;
        let num_digits = rhs.len() as u32;
        let denom = 10i64.pow(num_digits);
        let lhs: i64 = if lhs.is_empty() { 0 } else { lhs.parse().ok()? };
        let rhs: u64 = if rhs.is_empty() { 0 } else { rhs.parse().ok()? };
        let rhs = rhs as i64;
        debug_assert!(rhs < denom);
        let numer = lhs * denom + rhs;
        Some(RealValue::new(numer, denom))
    }
}

fn parse_number_sexpr(num: &SExpr) -> Result<RealValue, Message> {
    num.as_atom()
        .and_then(|e| parse_number(e.canonical_str()))
        .ok_or(num.invalid("expected number"))
}

/// Parses a timestamp as one of start, end or absolute time (2, 32.3, ...)
fn parse_timestamp(tp: &SExpr) -> Result<Timestamp, Message> {
    let timestamp = match tp.as_atom().map(|a| a.canonical_str()) {
        Some("start") => Some(Timestamp::from(TimeRef::ActionStart)),
        Some("end") => Some(Timestamp::from(TimeRef::ActionEnd)),
        Some(other) => parse_number(other).map(|number| Timestamp::new(TimeRef::Origin, number)),
        _ => None,
    };
    timestamp.ok_or_else(|| tp.invalid("expected a timestamp (start, end, 12.3, ...)"))
}

fn timed_sexpr(sexpr: &SExpr) -> Result<(TimeInterval, &SExpr), Message> {
    let mut items = sexpr
        .as_list_iter()
        .ok_or_else(|| sexpr.invalid("not a temporally qalified expression"))?;
    let first = items.pop_atom()?;
    let interval = match first.canonical_str() {
        "at" => {
            let second = items.pop_atom()?;
            match second.canonical_str() {
                "start" => TimeInterval::at(TimeRef::ActionStart),
                "end" => TimeInterval::at(TimeRef::ActionEnd),
                _ => return Err(second.invalid("expected `start` or `end`")),
            }
        }
        "over" => {
            items.pop_known_atom("all")?;
            TimeInterval::closed(TimeRef::ActionStart, TimeRef::ActionEnd)
        }
        _ => return Err(first.invalid("expected `at` or `over`")),
    };
    let expr = items.pop()?;
    if let Ok(x) = items.pop() {
        return Err(x.invalid("unexpected expression"));
    }
    Ok((interval, expr))
}

fn parse_timed(
    sexpr: &SExpr,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<(TimeInterval, ExprId), Message> {
    let (interval, expr) = timed_sexpr(sexpr)?;
    let expr = parse(expr, env, bindings)?;
    Ok((interval, expr))
}

fn parse_function(sym: &Sym) -> Option<Fun> {
    match sym.canonical_str() {
        "+" => Some(Fun::Plus),
        "-" => Some(Fun::Minus),
        "/" => Some(Fun::Div),
        "*" => Some(Fun::Mul),
        "and" => Some(Fun::And),
        "or" => Some(Fun::Or),
        "imply" => Some(Fun::Implies),
        "not" => Some(Fun::Not),
        "=" => Some(Fun::Eq),
        "<=" => Some(Fun::Leq),
        ">=" => Some(Fun::Geq),
        "<" => Some(Fun::Lt),
        ">" => Some(Fun::Gt),
        _ => None,
    }
}

fn parse_duration(dur: &SExpr, env: &mut Environment, bindings: &Rc<Bindings>) -> Result<Duration, Message> {
    // handle duration element from durative actions
    // currently, we only support constraint of the form `(= ?duration <i32>)`
    // TODO: extend durations constraints, to support the full PDDL spec
    let mut dur = dur.as_list_iter().unwrap();
    //Check for first two elements
    dur.pop_known_atom("=")?;
    dur.pop_known_atom("?duration")?;

    let dur_expr = dur.pop()?;
    let duration = parse(dur_expr, env, bindings)?;
    if let Ok(x) = dur.pop() {
        return Err(x.invalid("Unexpected"));
    }
    Type::REAL.accepts(duration, env).msg(env)?;
    Ok(Duration::Fixed(duration))
}

/*
    for t in &dom.types {
        types.push((t.symbol.clone(), t.tpe.clone()));
    }

    let ts = TypeHierarchy::new(types)?;
    let mut symbols: Vec<TypedSymbol> = prob.objects.clone();
    for c in &dom.constants {
        symbols.push(c.clone());
    }
    // predicates are symbols as well, add them to the table
    for p in &dom.predicates {
        symbols.push(TypedSymbol::new(&p.name, PREDICATE_TYPE));
    }
    for a in &dom.actions {
        symbols.push(TypedSymbol::new(&a.name, ACTION_TYPE));
    }
    for a in &dom.durative_actions {
        symbols.push(TypedSymbol::new(&a.name, DURATIVE_ACTION_TYPE));
    }
    for t in &dom.tasks {
        symbols.push(TypedSymbol::new(&t.name, ABSTRACT_TASK_TYPE));
    }
    for m in &dom.methods {
        symbols.push(TypedSymbol::new(&m.name, METHOD_TYPE));
    }
    //Add function name are symbols too
    for f in &dom.functions {
        symbols.push(TypedSymbol::new(&f.name, FUNCTION_TYPE));
    }

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(ts, symbols)?;

    let mut state_variables = Vec::with_capacity(dom.predicates.len() + dom.functions.len());
    for pred in &dom.predicates {
        let sym = symbol_table
            .id(&pred.name)
            .ok_or_else(|| pred.name.invalid("Unknown symbol"))?;
        let mut signature = Vec::with_capacity(pred.args.len() + 1);
        for a in &pred.args {
            let tpe = a.tpe.as_ref().unwrap_or(&top_type);
            let tpe = symbol_table
                .types
                .id_of(tpe)
                .ok_or_else(|| tpe.invalid("Unknown type"))?;
            signature.push(Type::Sym(tpe));
        }
        signature.push(Type::Bool); // return type (last one) is a boolean
        state_variables.push(Fluent {
            name: pred.name.clone(),
            sym,
            signature,
        })
    }
    for fun in &dom.functions {
        let sym = symbol_table
            .id(&fun.name)
            .ok_or_else(|| fun.name.invalid("Unknown symbol"))?;
        let mut signature = Vec::with_capacity(fun.args.len() + 1);
        for a in &fun.args {
            let tpe = a.tpe.as_ref().unwrap_or(&top_type);
            let tpe = symbol_table
                .types
                .id_of(tpe)
                .ok_or_else(|| tpe.invalid("Unknown type"))?;
            signature.push(Type::Sym(tpe));
        }
        // TODO: set to a fixed-point numeral of appropriate precision
        // return type (last one) is a int value
        signature.push(Type::UNBOUNDED_INT);
        state_variables.push(Fluent {
            name: fun.name.clone(),
            sym,
            signature,
        })
    }

    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

    let init_container = Container::Instance(0);
    // Initial chronicle construction
    let mut init_ch = Chronicle {
        kind: ChronicleKind::Problem,
        presence: Lit::TRUE,
        start: context.origin(),
        end: context.horizon(),
        name: vec![],
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
        cost: None,
    };

    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_model_atom_no_borrow = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        let atom = context
            .model
            .get_symbol_table()
            .id(atom.canonical_str())
            .ok_or_else(|| atom.invalid("Unknown atom"))?;
        let atom = context.typed_sym(atom);
        Ok(atom.into())
    };
    let as_model_atom = |atom: &sexpr::SAtom| as_model_atom_no_borrow(atom, &context);
    for goal in &prob.goal {
        // goal is expected to be a conjunction of the form:
        //  - `(and (= sv1 v1) (= sv2 = v2))`
        //  - `(= sv1 v1)`
        //  - `()`
        let goals = read_conjunction(goal, as_model_atom, context.model.get_symbol_table(), &context)?;
        for TermLoc(goal, loc) in goals {
            match goal {
                Term::Binding(sv, value) => init_ch.conditions.push(Condition {
                    start: init_ch.end,
                    end: init_ch.end,
                    state_var: sv,
                    value,
                }),
                _ => return Err(loc.invalid("Unsupported in goal expression").into()),
            }
        }
    }
    // If we have negative preconditions, we need to assume a closed world assumption.
    // Indeed, some preconditions might rely on initial facts being false
    let closed_world = dom.features.contains(&PddlFeature::NegativePreconditions);
    for (sv, val) in read_init(&prob.init, closed_world, as_model_atom, &context)? {
        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            transition_end: init_ch.start,
            min_mutex_end: Vec::new(),
            state_var: sv,
            operation: EffectOp::Assign(val),
        });
    }

    if let Some(ref task_network) = &prob.task_network {
        read_task_network(
            init_container,
            task_network,
            &as_model_atom_no_borrow,
            &mut init_ch,
            None,
            &mut context,
        )?;
    }

    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    for a in &dom.actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, a, &mut context)?;
        templates.push(template);
    }
    for a in &dom.durative_actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, a, &mut context)?;
        templates.push(template);
    }
    for m in &dom.methods {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, m, &mut context)?;
        templates.push(template);
    }

    let problem = Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    Ok(problem)
}

/// Transforms PDDL initial facts into binding of state variables to their values
/// If `closed_world` is true, then all predicates that are not given a true value will be set to false.
fn read_init(
    initial_facts: &[SExpr],
    closed_world: bool,
    as_model_atom: impl Fn(&sexpr::SAtom) -> Result<SAtom>,
    context: &Ctx,
) -> Result<Vec<(StateVar, Atom)>> {
    let mut facts: Vec<(StateVar, Atom)> = Vec::new();
    if closed_world {
        // closed world, every predicate that is not given a true value should be given a false value
        // to do this, we rely on the classical classical planning state
        let state_desc = World::new(context.model.get_symbol_table().clone(), &context.fluents)?;
        let mut s = state_desc.make_new_state();
        for init in initial_facts {
            let pred = read_sv(init, &state_desc)?;
            s.add(pred);
        }

        let sv_to_sv = |sv| -> StateVar {
            let syms = state_desc.sv_of(sv);
            let fluent = context.get_fluent(syms[0]).unwrap();
            let args = syms[1..].iter().map(|&sym| context.typed_sym(sym).into()).collect();
            StateVar::new(fluent.clone(), args)
        };

        for literal in s.literals() {
            let sv = sv_to_sv(literal.var());
            let val: Atom = literal.val().into();
            facts.push((sv, val));
        }
    } else {
        // open world, we only add to the initial facts the one explicitly given in the problem definition
        for e in initial_facts {
            match read_init_state(e, &as_model_atom, context)? {
                TermLoc(Term::Binding(sv, val), _) => facts.push((sv, val)),
                TermLoc(_, loc) => return Err(loc.invalid("Unsupported in initial facts").into()),
            }
        }
    }
    Ok(facts)
}

/// Transforms a PDDL action into a Chronicle template
///
/// # Parameters
///
/// - `c`: Identifier of the container that will be associated with the chronicle
/// - `pddl`: A view of a PDDL construct to be instantiated as a chronicle.
///   Can be, e.g., an instantaneous action, a method, ...
/// - `context`: Context in which the chronicle appears. Used to create new variables.
fn read_chronicle_template(
    c: Container,
    pddl: impl ChronicleTemplateView,
    context: &mut Ctx,
) -> Result<ChronicleTemplate> {
    let top_type = OBJECT_TYPE.into();

    // All parameters of the chronicle (!= from parameters of the action)
    // Must contain all variables that were created for this chronicle template
    // and should be replaced when instantiating the chronicle
    let mut params: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE.get(), prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end: FAtom = match pddl.kind() {
        ChronicleKind::Problem => panic!("unsupported case"),
        ChronicleKind::Method | ChronicleKind::DurativeAction => {
            let end =
                context
                    .model
                    .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE.get(), prez, c / VarType::ChronicleEnd);
            params.push(end.into());
            end.into()
        }
        ChronicleKind::Action => start, // non-durative actions are instantaneous
    };

    // name of the chronicle : name of the action + parameters
    let mut name: Vec<SAtom> = Vec::with_capacity(1 + pddl.parameters().len());
    let base_name = pddl.base_name();
    name.push(
        context
            .typed_sym(
                context
                    .model
                    .get_symbol_table()
                    .id(base_name)
                    .ok_or_else(|| base_name.invalid("Unknown atom"))?,
            )
            .into(),
    );
    // Process, the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for arg in pddl.parameters() {
        let tpe = arg.tpe.as_ref().unwrap_or(&top_type);
        let tpe = context
            .model
            .get_symbol_table()
            .types
            .id_of(tpe)
            .ok_or_else(|| tpe.invalid("Unknown atom"))?;
        let arg = context
            .model
            .new_optional_sym_var(tpe, prez, c / VarType::Parameter(arg.symbol.to_string()));
        params.push(arg.into());
        name.push(arg.into());
    }
    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_chronicle_atom_no_borrow = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        match pddl
            .parameters()
            .iter()
            .position(|arg| arg.symbol.canonical_str() == atom.canonical_str())
        {
            Some(i) => Ok(name[i + 1]),
            None => {
                let atom = context
                    .model
                    .get_symbol_table()
                    .id(atom.canonical_str())
                    .ok_or_else(|| atom.invalid("Unknown atom"))?;
                let atom = context.typed_sym(atom);
                Ok(atom.into())
            }
        }
    };
    let as_chronicle_atom = |atom: &sexpr::SAtom| -> Result<SAtom> { as_chronicle_atom_no_borrow(atom, context) };

    let task = if let Some(task) = pddl.task() {
        let mut task_name = Vec::with_capacity(task.arguments.len() + 1);
        task_name.push(as_chronicle_atom(&task.name)?);
        for task_arg in &task.arguments {
            task_name.push(as_chronicle_atom(task_arg)?);
        }
        task_name
    } else {
        // no explicit task (typical for a primitive action), use the name as the task
        name.clone()
    };

    // TODO: here the cost is simply 1 for any primitive action
    let cost = match pddl.kind() {
        ChronicleKind::Problem | ChronicleKind::Method => None,
        ChronicleKind::Action | ChronicleKind::DurativeAction => Some(1),
    };

    let mut ch = Chronicle {
        kind: pddl.kind(),
        presence: prez,
        start,
        end,
        name: name.iter().map(|satom| Atom::from(*satom)).collect(),
        task: Some(task.iter().map(|satom| Atom::from(*satom)).collect()),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
        cost,
    };

    for eff in pddl.effects() {
        if pddl.kind() != ChronicleKind::Action && pddl.kind() != ChronicleKind::DurativeAction {
            return Err(eff.invalid("Unexpected instantaneous effect").into());
        }
        let effects = read_conjunction(eff, as_chronicle_atom, context.model.get_symbol_table(), context)?;
        for TermLoc(term, loc) in effects {
            match term {
                Term::Binding(sv, val) => ch.effects.push(Effect {
                    transition_start: ch.end,
                    transition_end: ch.end + Time::EPSILON,
                    min_mutex_end: Vec::new(),
                    state_var: sv,
                    operation: EffectOp::Assign(val),
                }),
                _ => return Err(loc.invalid("Unsupported in action effects").into()),
            }
        }
    }

    for eff in pddl.timed_effects() {
        if pddl.kind() != ChronicleKind::Action && pddl.kind() != ChronicleKind::DurativeAction {
            return Err(eff.invalid("Unexpected effect").into());
        }
        // conjunction of effects of the form `(and (at-start (= sv1 v1)) (at-end (= sv2 v2)))`
        let effects = read_temporal_conjunction(eff, as_chronicle_atom, context)?;
        for TemporalTerm(qualification, term) in effects {
            match term.0 {
                Term::Binding(state_var, value) => match qualification {
                    TemporalQualification::AtStart => {
                        ch.effects.push(Effect {
                            transition_start: ch.start,
                            transition_end: ch.start + FAtom::EPSILON,
                            min_mutex_end: Vec::new(),
                            state_var,
                            operation: EffectOp::Assign(value),
                        });
                    }
                    TemporalQualification::AtEnd => {
                        ch.effects.push(Effect {
                            transition_start: ch.end,
                            transition_end: ch.end + FAtom::EPSILON,
                            min_mutex_end: Vec::new(),
                            state_var,
                            operation: EffectOp::Assign(value),
                        });
                    }
                    TemporalQualification::OverAll => {
                        return Err(term.1.invalid("Unsupported in action effects").into())
                    }
                },
                Term::Eq(_a, _b) => return Err(term.1.invalid("Unsupported in action effects").into()),
                Term::Neq(_a, _b) => return Err(term.1.invalid("Unsupported in action effects").into()),
            }
        }
    }

    // a common pattern in PDDL is to have two effect (not x) and (x) on the same state variable.
    // This is to force mutual exclusion on x. The semantics of PDDL have the negative effect applied first.
    // This is already enforced by our translation of a positive effect on x as `]start, end] x <- true`
    // Thus if we have both a positive effect and a negative effect on the same state variable,
    // we remove the negative one
    let positive_effects: HashSet<_> = ch
        .effects
        .iter()
        .filter(|e| e.operation == EffectOp::TRUE_ASSIGNMENT)
        .map(|e| (e.state_var.clone(), e.transition_end, e.transition_start))
        .collect();
    ch.effects.retain(|e| {
        e.operation != EffectOp::FALSE_ASSIGNMENT
            || !positive_effects.contains(&(e.state_var.clone(), e.transition_end, e.transition_start))
    });

    // TODO : check if work around still needed
    for cond in pddl.preconditions() {
        let conditions = read_conjunction(cond, as_chronicle_atom, context.model.get_symbol_table(), context)?;
        for TermLoc(term, _) in conditions {
            match term {
                Term::Binding(sv, val) => {
                    ch.conditions.push(Condition {
                        start: ch.start,
                        end: ch.start,
                        state_var: sv,
                        value: val,
                    });
                }
                Term::Eq(a, b) => ch.constraints.push(Constraint::eq(a, b)),
                Term::Neq(a, b) => ch.constraints.push(Constraint::neq(a, b)),
            }
        }
    }

    // handle duration element from durative actions
    if let Some(dur) = pddl.duration() {
        // currently, we only support constraint of the form `(= ?duration <i32>)`
        // TODO: extend durations constraints, to support the full PDDL spec
        let mut dur = dur.as_list_iter().unwrap();
        //Check for first two elements
        dur.pop_known_atom("=")?;
        dur.pop_known_atom("?duration")?;

        let dur_atom = dur.pop_atom()?;
        let duration = LinearSum::constant_int(
            dur_atom
                .canonical_str()
                .parse::<i32>()
                .map_err(|_| dur_atom.invalid("Expected an integer"))?,
        );
        ch.constraints.push(Constraint::duration(Duration::Fixed(duration)));
        if let Ok(x) = dur.pop() {
            return Err(x.invalid("Unexpected").into());
        }
    }

    //Handling temporal conditions
    for cond in pddl.timed_conditions() {
        let conditions = read_temporal_conjunction(cond, as_chronicle_atom, context)?;
        //let duration = read_duration()?;

        for TemporalTerm(qualification, term) in conditions {
            match term.0 {
                Term::Binding(state_var, value) => match qualification {
                    TemporalQualification::AtStart => {
                        ch.conditions.push(Condition {
                            start: ch.start,
                            end: ch.start,
                            state_var,
                            value,
                        });
                    }
                    TemporalQualification::AtEnd => {
                        ch.conditions.push(Condition {
                            start: ch.end,
                            end: ch.end,
                            state_var,
                            value,
                        });
                    }
                    TemporalQualification::OverAll => {
                        ch.conditions.push(Condition {
                            start: ch.start,
                            end: ch.end,
                            state_var,
                            value,
                        });
                    }
                },
                Term::Eq(a, b) => ch.constraints.push(Constraint::eq(a, b)),
                Term::Neq(a, b) => ch.constraints.push(Constraint::neq(a, b)),
            }
        }
    }

    if let Some(tn) = pddl.task_network() {
        read_task_network(c, tn, &as_chronicle_atom_no_borrow, &mut ch, Some(&mut params), context)?
    }

    let template = ChronicleTemplate {
        label: ChronicleLabel::Action(pddl.base_name().to_string()),
        parameters: params,
        chronicle: ch,
    };
    Ok(template)
}

/// An adapter to allow treating pddl actions and hddl methods identically
trait ChronicleTemplateView {
    fn kind(&self) -> ChronicleKind;
    fn base_name(&self) -> &Sym;
    fn parameters(&self) -> &[TypedSymbol];
    fn task(&self) -> Option<&pddl::Task>;
    fn duration(&self) -> Option<&SExpr>;
    fn preconditions(&self) -> &[SExpr];
    fn timed_conditions(&self) -> &[SExpr];
    fn effects(&self) -> &[SExpr];
    fn timed_effects(&self) -> &[SExpr];
    fn task_network(&self) -> Option<&pddl::TaskNetwork>;
}
impl ChronicleTemplateView for &pddl::Action {
    fn kind(&self) -> ChronicleKind {
        ChronicleKind::Action
    }
    fn base_name(&self) -> &Sym {
        &self.name
    }
    fn parameters(&self) -> &[TypedSymbol] {
        &self.args
    }
    fn task(&self) -> Option<&pddl::Task> {
        None
    }
    fn duration(&self) -> Option<&SExpr> {
        None
    }
    fn preconditions(&self) -> &[SExpr] {
        &self.pre
    }
    fn timed_conditions(&self) -> &[SExpr] {
        &[]
    }
    fn effects(&self) -> &[SExpr] {
        &self.eff
    }
    fn timed_effects(&self) -> &[SExpr] {
        &[]
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        None
    }
}
impl ChronicleTemplateView for &pddl::DurativeAction {
    fn kind(&self) -> ChronicleKind {
        ChronicleKind::DurativeAction
    }
    fn base_name(&self) -> &Sym {
        &self.name
    }
    fn parameters(&self) -> &[TypedSymbol] {
        &self.args
    }
    fn task(&self) -> Option<&pddl::Task> {
        None
    }
    fn duration(&self) -> Option<&SExpr> {
        Some(&self.duration)
    }
    fn preconditions(&self) -> &[SExpr] {
        &[]
    }
    fn timed_conditions(&self) -> &[SExpr] {
        &self.conditions
    }
    fn effects(&self) -> &[SExpr] {
        &[]
    }
    fn timed_effects(&self) -> &[SExpr] {
        &self.effects
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        None
    }
}
impl ChronicleTemplateView for &pddl::Method {
    fn kind(&self) -> ChronicleKind {
        ChronicleKind::Method
    }
    fn base_name(&self) -> &Sym {
        &self.name
    }
    fn parameters(&self) -> &[TypedSymbol] {
        &self.parameters
    }
    fn task(&self) -> Option<&pddl::Task> {
        Some(&self.task)
    }
    fn duration(&self) -> Option<&SExpr> {
        None
    }
    fn preconditions(&self) -> &[SExpr] {
        &self.precondition
    }
    fn timed_conditions(&self) -> &[SExpr] {
        &[]
    }
    fn effects(&self) -> &[SExpr] {
        &[]
    }
    fn timed_effects(&self) -> &[SExpr] {
        &[]
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        Some(&self.subtask_network)
    }
}

/// Parses a task network and adds its components (subtasks and constraints) to the target `chronicle.
/// All newly created variables (timepoints of the subtasks) are added to the new_variables buffer.
fn read_task_network(
    c: Container,
    tn: &pddl::TaskNetwork,
    as_chronicle_atom: &impl Fn(&sexpr::SAtom, &Ctx) -> Result<SAtom>,
    chronicle: &mut Chronicle,
    mut new_variables: Option<&mut Vec<Variable>>,
    context: &mut Ctx,
) -> Result<()> {
    // stores the start/end timepoints of each named task
    let mut named_task: HashMap<String, (FAtom, FAtom)> = HashMap::new();
    let top_type: Sym = OBJECT_TYPE.into();
    let presence = chronicle.presence;
    let mut local_params = Vec::new();
    for arg in &tn.parameters {
        let tpe = arg.tpe.as_ref().unwrap_or(&top_type);
        let tpe = context
            .model
            .get_symbol_table()
            .types
            .id_of(tpe)
            .ok_or_else(|| tpe.invalid("Unknown atom"))?;
        let arg = context
            .model
            .new_optional_sym_var(tpe, presence, c / VarType::Parameter(arg.symbol.to_string()));
        if let Some(new_variables) = &mut new_variables {
            new_variables.push(arg.into());
        }
        local_params.push(arg);
    }

    // consider task network parameters in following expressions.
    let as_chronicle_atom = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        match tn
            .parameters
            .iter()
            .position(|arg| arg.symbol.canonical_str() == atom.canonical_str())
        {
            Some(i) => Ok(local_params[i].into()),
            None => as_chronicle_atom(atom, context),
        }
    };

    // creates a new subtask. This will create new variables for the start and end
    // timepoints of the task and push the `new_variables` vector, if any.
    let mut make_subtask = |t: &pddl::Task, task_id: u32| -> Result<SubTask> {
        let id = t.id.as_ref().map(|id| id.canonical_string());
        // get the name + parameters of the task
        let mut task_name = Vec::with_capacity(t.arguments.len() + 1);
        task_name.push(as_chronicle_atom(&t.name, context)?);
        for param in &t.arguments {
            task_name.push(as_chronicle_atom(param, context)?);
        }
        let task_name = task_name.iter().map(|satom| Atom::from(*satom)).collect();

        // create timepoints for the subtask
        let start = context.model.new_optional_fvar(
            0,
            INT_CST_MAX,
            TIME_SCALE.get(),
            presence,
            c / VarType::TaskStart(task_id),
        );
        let end = context.model.new_optional_fvar(
            0,
            INT_CST_MAX,
            TIME_SCALE.get(),
            presence,
            c / VarType::TaskEnd(task_id),
        );
        if let Some(ref mut params) = new_variables {
            params.push(start.into());
            params.push(end.into());
        }
        let start = FAtom::from(start);
        let end = FAtom::from(end);
        if let Some(name) = id.as_ref() {
            named_task.insert(name.clone(), (start, end));
        }
        Ok(SubTask {
            id,
            start,
            end,
            task_name,
        })
    };
    let mut task_id = 0;
    for t in &tn.unordered_tasks {
        let t = make_subtask(t, task_id)?;
        chronicle.subtasks.push(t);
        task_id += 1;
    }

    // parse all ordered tasks, adding precedence constraints between subsequent ones
    let mut previous_end = None;
    for t in &tn.ordered_tasks {
        let t = make_subtask(t, task_id)?;

        if let Some(previous_end) = previous_end {
            chronicle.constraints.push(Constraint::lt(previous_end, t.start))
        }
        previous_end = Some(t.end);
        chronicle.subtasks.push(t);
        task_id += 1;
    }
    for ord in &tn.orderings {
        let first_end = named_task
            .get(ord.first_task_id.canonical_str())
            .ok_or_else(|| ord.first_task_id.invalid("Unknown task id"))?
            .1;
        let second_start = named_task
            .get(ord.second_task_id.canonical_str())
            .ok_or_else(|| ord.second_task_id.invalid("Unknown task id"))?
            .0;
        chronicle.constraints.push(Constraint::lt(first_end, second_start));
    }
    for c in &tn.constraints {
        // treat constraints exactly as we treat preconditions
        let as_chronicle_atom = |x: &sexpr::SAtom| as_chronicle_atom(x, context);
        let conditions = read_conjunction(c, as_chronicle_atom, context.model.get_symbol_table(), context)?;
        for TermLoc(term, _) in conditions {
            match term {
                Term::Binding(sv, val) => {
                    chronicle.conditions.push(Condition {
                        start: chronicle.start,
                        end: chronicle.start,
                        state_var: sv,
                        value: val,
                    });
                }
                Term::Eq(a, b) => chronicle.constraints.push(Constraint::eq(a, b)),
                Term::Neq(a, b) => chronicle.constraints.push(Constraint::neq(a, b)),
            }
        }
    }

    Ok(())
}

enum Term {
    Binding(StateVar, Atom),
    Eq(Atom, Atom),
    Neq(Atom, Atom),
}

/// A Term, with its location in the input file (for error handling).
struct TermLoc(Term, Loc);
struct TemporalTerm(TemporalQualification, TermLoc);

/// Temporal qualification that can be applied to an expression.
enum TemporalQualification {
    AtStart,
    OverAll,
    AtEnd,
}

impl std::str::FromStr for TemporalQualification {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "at start" => Ok(TemporalQualification::AtStart),
            "over all" => Ok(TemporalQualification::OverAll),
            "at end" => Ok(TemporalQualification::AtEnd),
            _ => Err(format!("Unknown temporal qualification: {s}")),
        }
    }
}

fn instances_of(tpe: &Sym, syms: &SymbolTable) -> Result<Vec<SAtom>> {
    let mut instances = Vec::new();
    let tpe = syms.types.id_of(tpe).context("Unknown type")?;
    for s in syms.instances_of_type(tpe) {
        instances.push(SAtom::new_constant(s, syms.type_of(s)));
    }
    Ok(instances)
}

fn read_conjunction(
    e: &SExpr,
    t: impl Fn(&sexpr::SAtom) -> Result<SAtom>,
    syms: &SymbolTable,
    context: &Ctx,
) -> Result<Vec<TermLoc>> {
    let mut result = Vec::new();
    read_conjunction_impl(e, &t, &mut result, syms, context)?;
    Ok(result)
}

fn read_conjunction_impl(
    e: &SExpr,
    t: &dyn Fn(&sexpr::SAtom) -> Result<SAtom>,
    out: &mut Vec<TermLoc>,
    syms: &SymbolTable,
    context: &Ctx,
) -> Result<()> {
    if let Some(l) = e.as_list_iter() {
        if l.is_empty() {
            return Ok(()); // empty conjunction
        }
    }
    if let Some(conjuncts) = e.as_application("and") {
        for c in conjuncts.iter() {
            read_conjunction_impl(c, t, out, syms, context)?;
        }
    } else if let Some(conjuncts) = e.as_application("forall") {
        let mut params = conjuncts[0].as_list_iter().context("expected parameters")?;
        let params = consume_typed_symbols(&mut params)?;
        let expr = &conjuncts[1];
        assert_eq!(params.len(), 1, "Only support a single argument per forall.");
        let ts = &params[0];
        let var = &ts.symbol;
        let default_type = OBJECT_TYPE.into();
        let tpe = ts.tpe.as_ref().unwrap_or(&default_type);
        for instance in instances_of(tpe, syms).context("Unknown type")? {
            let t = |x: &sexpr::SAtom| -> Result<SAtom> {
                if x.canonical_str() == var.canonical_str() {
                    Ok(instance)
                } else {
                    t(x)
                }
            };
            read_conjunction_impl(expr, &t, out, syms, context)?;
        }
    } else {
        // should be directly a predicate
        out.push(read_possibly_negated_term(e, t, context)?);
    }
    Ok(())
}

fn read_temporal_conjunction(
    e: &SExpr,
    t: impl Fn(&sexpr::SAtom) -> Result<SAtom>,
    context: &Ctx,
) -> Result<Vec<TemporalTerm>> {
    let mut result = Vec::new();
    read_temporal_conjunction_impl(e, &t, &mut result, context)?;
    Ok(result)
}

// So for a temporal conjunctions of syntax
// (and (at start ?x) (at start ?y))
// we want to place in `out`:
//  - (TemporalQualification::AtStart, term_of(?x))
//  - (TemporalQualification::AtStart, term_of(?y))
// Vector(TemporalQualification, respective sv, respective atom)
fn read_temporal_conjunction_impl(
    e: &SExpr,
    t: &impl Fn(&sexpr::SAtom) -> Result<SAtom>,
    out: &mut Vec<TemporalTerm>,
    context: &Ctx,
) -> Result<()> {
    if let Some(l) = e.as_list_iter() {
        if l.is_empty() {
            return Ok(()); // empty conjunction
        }
    }
    if let Some(conjuncts) = e.as_application("and") {
        for c in conjuncts.iter() {
            read_temporal_conjunction_impl(c, t, out, context)?;
        }
    } else {
        // should be directly a temporaly qualified predicate
        out.push(read_temporal_term(e, t, context)?);
    }
    Ok(())
}

// Parses something of the form: (at start ?x)
// To retrieve the term (`?x`) and its temporal qualification (`at start`)
fn read_temporal_term(expr: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>, context: &Ctx) -> Result<TemporalTerm> {
    let mut expr = expr
        .as_list_iter()
        .ok_or_else(|| expr.invalid("Expected a valid term"))?;
    let atom = expr.pop_atom()?.canonical_str(); // "at" or "over"
    let atom = atom.to_owned() + " " + expr.pop_atom()?.canonical_str(); // "at start", "at end", or "over all"

    let qualification = TemporalQualification::from_str(atom.as_str()).map_err(|e| expr.invalid(e))?;
    // Read term here
    let term = expr.pop()?; // the "term" in (at start "term")
    let term = read_possibly_negated_term(term, t, context)?;
    Ok(TemporalTerm(qualification, term))
}

fn read_possibly_negated_term(e: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>, context: &Ctx) -> Result<TermLoc> {
    if let Some([to_negate]) = e.as_application("not") {
        let TermLoc(t, _) = read_term(to_negate, &t, context)?;
        let negated = match t {
            Term::Binding(sv, value) => {
                if let Ok(value) = Lit::try_from(value) {
                    Term::Binding(sv, Atom::from(!value))
                } else {
                    return Err(to_negate.invalid("Could not apply 'not' to this expression").into());
                }
            }
            Term::Eq(a, b) => Term::Neq(a, b),
            Term::Neq(a, b) => Term::Eq(a, b),
        };
        Ok(TermLoc(negated, e.loc()))
    } else {
        // should be directly a predicate
        Ok(read_term(e, &t, context)?)
    }
}

fn to_state_variable(mut atoms: Vec<SAtom>, context: &Ctx) -> Result<StateVar> {
    let fluent = if let SAtom::Cst(s) = atoms.remove(0) {
        context.get_fluent(s.sym).context("Not a fluent")?.clone()
    } else {
        bail!("")
    };
    Ok(StateVar::new(fluent, atoms))
}

fn read_init_state(expr: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>, context: &Ctx) -> Result<TermLoc> {
    let mut l = expr.as_list_iter().ok_or_else(|| expr.invalid("Expected a term"))?;
    if let Some(head) = l.peek() {
        let head = head.as_atom().ok_or_else(|| head.invalid("Expected an atom"))?;
        let term = match head.canonical_str() {
            "=" => {
                l.pop_known_atom("=")?;
                let expr = l.pop()?.as_list_iter().unwrap();
                let mut sv = Vec::with_capacity(l.len());
                for e in expr {
                    let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
                    let atom = t(atom)?;
                    sv.push(atom);
                }
                let value = l
                    .pop_atom()?
                    .clone()
                    .canonical_str()
                    .parse::<i32>()
                    .map_err(|_| l.invalid("Expected an integer"))?;
                if let Some(unexpected) = l.next() {
                    return Err(unexpected.invalid("Unexpected expr").into());
                }
                Term::Binding(to_state_variable(sv, context)?, Atom::Int(value.into()))
            }
            _ => {
                let mut sv = Vec::with_capacity(l.len());
                for e in l {
                    let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
                    let atom = t(atom)?;
                    sv.push(atom);
                }
                Term::Binding(to_state_variable(sv, context)?, true.into())
            }
        };
        Ok(TermLoc(term, expr.loc()))
    } else {
        Err(l.loc().end().invalid("Expected a term").into())
    }
}

fn read_term(expr: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>, context: &Ctx) -> Result<TermLoc> {
    let mut l = expr.as_list_iter().ok_or_else(|| expr.invalid("Expected a term"))?;
    if let Some(head) = l.peek() {
        let head = head.as_atom().ok_or_else(|| head.invalid("Expected an atom"))?;
        let term = match head.canonical_str() {
            "=" => {
                l.pop_known_atom("=")?;
                let a = l.pop_atom()?.clone();
                let b = l.pop_atom()?.clone();
                if let Some(unexpected) = l.next() {
                    return Err(unexpected.invalid("Unexpected expr").into());
                }
                Term::Eq(t(&a)?.into(), t(&b)?.into())
            }
            _ => {
                let mut sv = Vec::with_capacity(l.len());
                for e in l {
                    let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
                    let atom = t(atom)?;
                    sv.push(atom);
                }
                Term::Binding(to_state_variable(sv, context)?, true.into())
            }
        };
        Ok(TermLoc(term, expr.loc()))
    } else {
        Err(l.loc().end().invalid("Expected a term").into())
    }
}

fn read_sv(e: &SExpr, desc: &World) -> Result<SvId> {
    let p = e.as_list().context("Expected s-expression")?;
    let atoms: Result<Vec<_>, ErrLoc> = p
        .iter()
        .map(|e| e.as_atom().ok_or_else(|| e.invalid("Expected atom")))
        .collect();
    let atom_ids: Result<Vec<_>, ErrLoc> = atoms?
        .iter()
        .map(|atom| {
            desc.table
                .id(atom.canonical_str())
                .ok_or_else(|| atom.invalid("Unknown atom"))
        })
        .collect();
    let atom_ids = atom_ids?;
    desc.sv_id(atom_ids.as_slice()).with_context(|| {
        format!(
            "Unknown predicate {} (wrong number of arguments or badly typed args ?)",
            desc.table.format(&atom_ids)
        )
    })
}
*/

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_num() {
        assert_eq!(parse_number("919.7"), Some(RealValue::new(9197, 10)));
        assert_eq!(parse_number("0.9197"), Some(RealValue::new(9197, 10000)));
        assert_eq!(parse_number("919"), Some(RealValue::new(919, 1)));
        assert_eq!(parse_number("919."), Some(RealValue::new(919, 1)));
    }
}
