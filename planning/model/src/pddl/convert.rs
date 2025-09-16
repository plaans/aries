use std::rc::Rc;

use errors::*;
use itertools::Itertools;
use pddl::sexpr::SExpr;
use smallvec::SmallVec;

use crate::pddl::sexpr::ListIter;
use crate::*;

use super::input::Sym;
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
                return Err(
                    Message::from(second_parent.invalid("unexpected second parent type")).info(&tpe.symbol, "for type")
                );
            }
        }
    }
    Ok(types)
}

pub fn build_model(dom: &Domain, prob: &Problem) -> anyhow::Result<Model> {
    // top types in pddl

    let types = user_types(dom)?;
    let types = Types::new(types);
    let mut model = Model::new(types);

    for pred in &dom.predicates {
        let parameters = parse_parameters(&pred.args, &model.env.types).msg(&model.env)?;
        model.env.fluents.add_fluent(&pred.name, parameters, Type::Bool)?;
    }

    for func in &dom.functions {
        let parameters = parse_parameters(&func.args, &model.env.types).msg(&model.env)?;
        model.env.fluents.add_fluent(&func.name, parameters, Type::Real)?;
    }

    let mut objects = Objects::new();

    for obj in dom.constants.iter().chain(prob.objects.iter()) {
        let tpe = match obj.tpe.as_slice() {
            [] => model.env.types.top_user_type(),
            [tpe] => model.env.types.get_user_type(tpe).msg(&model.env)?,
            [_, tpe, ..] => return Err(tpe.invalid("object with more than one type").msg().into()),
        };
        objects.add_object(&obj.symbol, tpe)?;
    }

    let bindings = Rc::new(Bindings::objects(&objects));

    let has_at_fluent = model.env.fluents.get_by_name("at").is_some();
    for init in &prob.init {
        let (timestamp, expr) = if !has_at_fluent && let Some([tp, init]) = init.as_application("at") {
            // (at 54 (loc r1 l2))
            if let Some(tp) = tp.as_atom()
                && let Some(num) = parse_number(tp.canonical_str())
            {
                let tp = Timestamp::new(TimeRef::Origin, num);
                (tp, init)
            } else {
                return Err(tp.loc().invalid("expected an absolute time").msg().into());
            }
        } else {
            (Timestamp::ORIGIN, init)
        };
        let e = into_effect(timestamp, expr, &mut model.env, &bindings)?;
        model.init.push(e);
    }

    for g in &prob.goal {
        let sub_goals = conjuncts(g);
        for g in sub_goals {
            if is_preference(g) {
                let pref = parse_preference(g, true, &mut model.env, &bindings)?;
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
                let pref = parse_preference(c, false, &mut model.env, &bindings)?;
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

    for a in &dom.actions {
        let name: crate::Sym = a.name.clone();
        let action = into_action(a, &mut model.env, &bindings).with_info(|| name.info("when parsing action"))?;
        model.actions.add(action)?;
    }

    for a in &dom.durative_actions {
        let name: crate::Sym = a.name.clone();
        let action =
            into_durative_action(a, &mut model.env, &bindings).with_info(|| name.info("when parsing action"))?;
        model.actions.add(action)?;
    }

    println!("{model}");

    Ok(model)
}

fn into_action(a: &pddl::Action, env: &mut Environment, bindings: &Rc<Bindings>) -> Result<Action, Message> {
    let parameters = parse_parameters(&a.args, &env.types).msg(env)?;

    let bindings = Rc::new(Bindings::stacked(&parameters, bindings));

    let mut action = Action::instantaneous(&a.name, parameters);

    for c in &a.pre {
        for c in conjuncts(c) {
            let c = parse(c, env, &bindings)?;
            action.conditions.push(Condition::at(action.start(), c));
        }
    }

    for e in &a.eff {
        for e in conjuncts(e) {
            let e = into_effect(action.end(), e, env, &bindings)?;
            action.effects.push(e);
        }
    }
    Ok(action)
}
fn into_durative_action(
    a: &pddl::DurativeAction,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Action, Message> {
    let parameters = parse_parameters(&a.args, &env.types).msg(env)?;

    let bindings = Rc::new(Bindings::stacked(&parameters, bindings));
    let duration = parse_duration(&a.duration, env, &bindings)?;

    let mut action = Action::new(&a.name, parameters, duration);

    for c in &a.conditions {
        for c in conjuncts(c) {
            let (itv, c) = parse_timed(c, env, &bindings).with_info(|| c.loc().info("when parsing condition"))?;
            action.conditions.push(Condition::over(itv, c));
        }
    }

    for e in &a.effects {
        for e in conjuncts(e) {
            let (itv, eff) = timed_sexpr(e)?;
            let tp = if itv == TimeInterval::at(TimeRef::Start) {
                TimeRef::Start
            } else if itv == TimeInterval::at(TimeRef::End) {
                TimeRef::End
            } else {
                return Err(Message::error("Invalid temporal qualifier for effect")
                    .snippet(e.loc().error("Requires a timepoint `at start` or `at end`")));
            };
            let e = into_effect(tp, eff, env, &bindings)?;
            action.effects.push(e);
        }
    }
    Ok(action)
}

fn into_effect(
    time: impl Into<Timestamp>,
    expr: &SExpr,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Effect, Message> {
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
        let contradiction = env.intern(Expr::Bool(false), None).msg(env)?;
        let sv = parse_sv(arg, Some(Type::Bool), env, bindings)?;
        Ok(Effect::assignement(time.into(), sv, contradiction))
    } else if let Some([sv, val]) = expr.as_application("=") {
        let sv = parse_sv(sv, None, env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(Effect::assignement(time.into(), sv, val))
    } else if let Some([sv, val]) = expr.as_application("assign") {
        let sv = parse_sv(sv, None, env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(Effect::assignement(time.into(), sv, val))
    } else if let Some([sv, val]) = expr.as_application("increase") {
        // (increase (fuel-level r1) 2)
        let sv = parse_sv(sv, Some(Type::Real), env, bindings)?;
        let val = parse(val, env, bindings)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(Effect::increase(time.into(), sv, val))
    } else if let Some([sv, val]) = expr.as_application("decrease") {
        // (increase (fuel-level r1) 2)
        let sv = parse_sv(sv, Some(Type::Real), env, bindings)?;
        let val = parse(val, env, bindings)?;
        let neg_val = env
            .intern(
                Expr::App(Fun::Minus, SmallVec::from_slice(&[val])),
                env.node(val).span().cloned(),
            )
            .msg(env)?;
        env.node(sv.fluent).tpe().accepts(val, env).msg(env)?;
        Ok(Effect::increase(time.into(), sv, neg_val))
    } else {
        let tautology = env.intern(Expr::Bool(true), None).msg(env)?;
        let sv = parse_sv(expr, Some(Type::Bool), env, bindings)?;
        Ok(Effect::assignement(time.into(), sv, tautology))
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
            Err(err) => parse_number(atom.canonical_str()).map(Expr::Real).ok_or(err)?,
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
                    return Err(x.invalid("Unexpected argument to total-time").into());
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
                return Err(f.invalid("unknown atom").into());
            }
        }
    };
    env.intern(expr, sexpr.loc()).msg(env).map_err(|e| {
        e.snippet(
            sexpr
                .loc()
                .annotate(annotate_snippets::Level::Info, "when parsing expression"),
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
            return Err(tp.invalid("expected `end`").into());
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
        Err(sexpr.invalid("invalid goal expression").into())
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
fn parse_preference(
    sexpr: &SExpr,
    at_horizon: bool,
    env: &mut Environment,
    bindings: &Rc<Bindings>,
) -> Result<Preference, Message> {
    if let Some([id, goal]) = sexpr.as_application("preference") {
        let id = id
            .as_atom()
            .ok_or(id.invalid("expected preference identifier"))
            .cloned()?;
        let goal = parse_goal(goal, at_horizon, env, bindings)?;
        Ok(Preference::new(id, goal))
    } else if let Some([vars, pref]) = sexpr.as_application("forall") {
        let vars = parse_var_list(vars, env)?;
        let bindings = Rc::new(Bindings::stacked(&vars, bindings));
        parse_preference(pref, at_horizon, env, &bindings).map(|pref| pref.forall(vars))
    } else {
        Err(sexpr.invalid("malformed preference").into())
    }
}

/// Parse a number number ("32", "-3", "3.14", -323.3")
fn parse_number(decimal_str: &str) -> Option<RealValue> {
    if let Ok(i) = decimal_str.parse::<IntValue>() {
        Some(RealValue::new(i, 1))
    } else {
        let (lhs, rhs) = decimal_str.split_once(".")?;
        let denom = rhs.len() as i64;
        let lhs: i64 = lhs.parse().ok()?;
        let rhs: u64 = rhs.parse().ok()?;
        let numer = lhs * denom + (rhs as i64);
        Some(RealValue::new(numer, denom))
    }
}

fn parse_number_sexpr(num: &SExpr) -> Result<RealValue, Message> {
    num.as_atom()
        .and_then(|e| parse_number(e.canonical_str()))
        .ok_or(num.invalid("expected number").into())
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
                "start" => TimeInterval::at(TimeRef::Start),
                "end" => TimeInterval::at(TimeRef::End),
                _ => return Err(second.invalid("expected `start` or `end`").into()),
            }
        }
        "over" => {
            items.pop_known_atom("all")?;
            TimeInterval::closed(TimeRef::Start, TimeRef::End)
        }
        _ => return Err(first.invalid("expected `at` or `over`").into()),
    };
    let expr = items.pop()?;
    if let Ok(x) = items.pop() {
        return Err(x.invalid("unexpected expression").into());
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
    match sym.symbol.as_str() {
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
        return Err(x.invalid("Unexpected").into());
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
