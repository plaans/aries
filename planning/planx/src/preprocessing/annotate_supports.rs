//! Annotation of the lifted *causal support* relation: which effect may support which requirement.
//!
//! A *requirement* is a simple requirement (see [`SimpleRequirement`]) of an action condition or of a simple goal.
//! An effect `sv := v` **may support** a requirement on the same fluent if the arguments of both may denote the same
//! state variable and `v` may be the required value (based on the possible values of parameters, see
//! [`ParamDomains`]).
//!
//! The pass computes:
//!  - for each effect, the requirements it may support ([`Supports`]):
//!     - effects on *opaque* fluents (see [`FluentProperties::opaque`]) may be useful for anything,
//!       except numeric changes in a direction that is never helpful (see [`NumericProperties`]);
//!     - erasures and *borrows* (see [`annotate_transitions`](super::annotate_transitions)) support nothing:
//!       a borrow restores the value that held just before it, so it never makes anything true that was not already.
//!       This also applies to the restore of a *lock* (a state variable erased by a transition then restored to the
//!       same value by the same action) when all other effects on the fluent are guarded (transitions), lock
//!       restores or unguarded assignments of the same constant: while erased, the state variable can then not be
//!       given another value;
//!     - a supported condition is flagged as `undo` if it is part of a transition that restores the value consumed by
//!       the transition of the effect (e.g. `switch_off(?i)` with respect to `switch_on(?i)`).
//!  - for each condition made of a single simple requirement, the effects that may support it ([`Supporters`]).
//!  - for each action, its [`ActionStatus`]:
//!     - *reachable*: least fixpoint where an action is reachable if each of its parameters may take a value, and each
//!       of its simple requirements may be supported by the problem definition or by an effect of a reachable action
//!       (or by an earlier effect of the same instance);
//!     - *useful*: least fixpoint where an action is useful if it is reachable and has an effect that may support an
//!       opaque read, a goal, or a condition of a useful action.
//!       In hierarchical models, all reachable actions are considered useful.
//!
//! The annotations refer to elements of other actions and to goals: they are only valid as long as the model is not
//! modified. This pass should thus be the last transformation applied to the model (or be recomputed after any
//! other transformation).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::preprocessing::analysis::{
    ActionTiming, NumericBounds, ParamDomains, SimpleRequirement, SimpleValue, StaticFunction, StaticFunctions, Term,
    conjuncts, fluents_modified_by_actions, numeric_properties, opaque_fluents, simple_goal, simple_requirement,
};
use crate::*;

/// Annotates the supports, supporters and status of all actions (see the module documentation).
/// Any previous support annotation is replaced.
pub fn annotate_supports(model: &mut Model) -> Res<()> {
    let (result, goals) = SupportAnalysis::new(model).run();
    for goal in model.goals.iter_mut() {
        goal.annotations.supporters = goals.get(&goal.id).cloned();
    }
    for (name, res) in result {
        let action = model.actions.get_action_mut(&name).unwrap();
        action.annotations.status = Some(res.status);
        for c in action.conditions.iter_mut() {
            c.annotations.supporters = res.supporters.get(&c.id).cloned();
        }
        for e in action.effects.iter_mut() {
            e.annotations.supports = res.supports.get(&e.id).cloned();
        }
    }
    Ok(())
}

/// Supporters of the simple requirements of each goal.
type GoalSupporters = HashMap<GoalId, Vec<Supporters>>;

/// Annotations computed for an action.
struct ActionResult {
    status: ActionStatus,
    supporters: HashMap<ConditionId, Supporters>,
    supports: HashMap<EffectId, Supports>,
}

/// Over-approximation of the values an argument or value may take.
#[derive(Clone, Debug)]
enum Dom {
    Any,
    Objects(Rc<HashSet<Sym>>),
    Bool(bool),
    Real(RealValue),
}

impl Dom {
    fn compatible(&self, other: &Dom) -> bool {
        match (self, other) {
            (Dom::Any, _) | (_, Dom::Any) => true,
            (Dom::Objects(a), Dom::Objects(b)) => {
                let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
                small.iter().any(|x| large.contains(x))
            }
            (Dom::Bool(a), Dom::Bool(b)) => a == b,
            (Dom::Real(a), Dom::Real(b)) => a == b,
            _ => false,
        }
    }
}

/// An atom (state variable and value) whose arguments and value are over-approximated.
#[derive(Clone, Debug)]
struct Pattern {
    fluent: FluentId,
    args: Vec<Dom>,
    /// `None` for an erasure (no value).
    value: Option<Dom>,
}

impl Pattern {
    /// True if an effect with this pattern may support a requirement with the other pattern.
    fn may_support(&self, req: &Pattern) -> bool {
        self.fluent == req.fluent
            && self.args.len() == req.args.len()
            && self.args.iter().zip(&req.args).all(|(a, b)| a.compatible(b))
            && match (&self.value, &req.value) {
                (Some(v), Some(r)) => v.compatible(r),
                _ => false,
            }
    }
}

/// Possible values of the parameters in some scope (action parameters and quantified variables).
#[derive(Clone, Default)]
struct Scope {
    params: HashMap<Sym, Rc<HashSet<Sym>>>,
}

impl Scope {
    fn term(&self, t: &Term) -> Dom {
        match t {
            Term::Object(o) => Dom::Objects(Rc::new([o.clone()].into())),
            Term::Param(p) => self.params.get(p).cloned().map(Dom::Objects).unwrap_or(Dom::Any),
        }
    }

    fn value(&self, v: &SimpleValue) -> Dom {
        match v {
            SimpleValue::Bool(b) => Dom::Bool(*b),
            SimpleValue::Real(r) => Dom::Real(*r),
            SimpleValue::Term(t) => self.term(t),
        }
    }

    fn requirement(&self, r: &SimpleRequirement) -> Pattern {
        Pattern {
            fluent: r.fluent,
            args: r.args.iter().map(|t| self.term(t)).collect(),
            value: Some(self.value(&r.value)),
        }
    }

    fn with_vars(&self, vars: &[Param], env: &Environment) -> Scope {
        let mut scope = self.clone();
        for v in vars {
            scope.params.insert(v.name().clone(), Rc::new(objects_of(v.tpe(), env)));
        }
        scope
    }
}

fn objects_of(tpe: &Type, env: &Environment) -> HashSet<Sym> {
    (env.objects.iter())
        .filter(|o| Type::from(o.tpe()).is_subtype_of(tpe))
        .map(|o| o.name().clone())
        .collect()
}

/// Pattern of an effect, `None` if it is an increase or decrease (that may support any requirement on its fluent).
fn effect_pattern(eff: &Effect, scope: &Scope, env: &Environment) -> Pattern {
    let scope = scope.with_vars(&eff.universal_quantification, env);
    let e = &eff.effect_expression;
    let args = (e.state_variable.arguments.iter())
        .map(|&a| Term::of(a, env).map(|t| scope.term(&t)).unwrap_or(Dom::Any))
        .collect();
    let value = match e.operation {
        EffectOp::Assign(v) => Some(SimpleValue::of(v, env).map(|v| scope.value(&v)).unwrap_or(Dom::Any)),
        EffectOp::Increase(_) | EffectOp::Decrease(_) => Some(Dom::Any),
        EffectOp::Erase => None,
    };
    Pattern {
        fluent: e.state_variable.fluent,
        args,
        value,
    }
}

/// A simple requirement of an action condition or of a goal.
struct Requirement {
    owner: Owner,
    req: SimpleRequirement,
    pattern: Pattern,
}

#[derive(Clone, Debug)]
enum Owner {
    /// Action (index) and condition
    Action(usize, ConditionId),
    Goal(GoalId),
}

/// True if the effect is the *restore* of a lock: an assignment `sv := v` such that the action has an earlier
/// transition that requires `sv = v` and erases `sv` (e.g. `available(?c)` erased at start and restored at end).
///
/// If no other action can change `sv` while it is erased, the action leaves `sv` unchanged: the restore never makes
/// anything true that was not already, exactly as a borrow.
fn lock_restore(action: &Action, eff: &Effect, timing: &ActionTiming, env: &Environment) -> bool {
    let e = &eff.effect_expression;
    let (EffectOp::Assign(v), None) = (e.operation.clone(), e.condition) else {
        return false;
    };
    if !eff.universal_quantification.is_empty() {
        return false;
    }
    let Some(args) = Term::all_of(&e.state_variable.arguments, env) else {
        return false;
    };
    let value = SimpleValue::of(v, env);
    action.transitions(env).any(|t| {
        let te = &t.effect.effect_expression;
        t.value.is_none()
            && te.state_variable.fluent == e.state_variable.fluent
            && Term::all_of(&te.state_variable.arguments, env).as_ref() == Some(&args)
            && value.is_some()
            && SimpleValue::required(t.required, env) == value
            && timing.le(te.transition_start, e.transition_start)
    })
}

/// The value assigned by the effect if it is a constant (not a parameter).
fn constant_value(eff: &Effect, env: &Environment) -> Option<SimpleValue> {
    let e = &eff.effect_expression;
    match e.operation {
        EffectOp::Assign(v) if e.condition.is_none() && eff.universal_quantification.is_empty() => {
            SimpleValue::of(v, env).filter(|v| !matches!(v, SimpleValue::Term(Term::Param(_))))
        }
        _ => None,
    }
}

/// Information on an effect of an action.
struct EffectInfo {
    id: EffectId,
    pattern: Pattern,
    /// True if the effect is part of a borrow, or is the restore of a lock that cannot be changed by other actions
    /// (see [`lock_restore`]).
    borrow: bool,
    /// For an effect that is part of a (non-borrow) transition: the required and assigned values.
    transition: Option<(SimpleValue, SimpleValue)>,
}

struct SupportAnalysis<'a> {
    model: &'a Model,
    actions: Vec<&'a Action>,
    timings: Vec<ActionTiming>,
    effects: Vec<Vec<EffectInfo>>,
    requirements: Vec<Requirement>,
    /// For each action, the (index of) the requirements of each of its conditions.
    action_requirements: Vec<Vec<usize>>,
    /// For each action, true if one of its parameters cannot take any value.
    empty_domain: Vec<bool>,
    init: Vec<Pattern>,
    opaque: HashSet<FluentId>,
    numeric: HashMap<FluentId, NumericProperties>,
    bounds: NumericBounds<'a>,
    /// Static requirements of each action (used to relate parameters of distinct actions).
    statics: Vec<Vec<SimpleRequirement>>,
    functions: StaticFunctions,
    /// For each effect, true if it supports nothing (computed without undo detection).
    useless: Vec<Vec<bool>>,
}

impl<'a> SupportAnalysis<'a> {
    fn new(model: &'a Model) -> Self {
        let env = &model.env;
        let actions = model.actions.iter().collect::<Vec<_>>();
        let bounds = NumericBounds::new(model);
        let domains = ParamDomains::new(model);
        let modified = fluents_modified_by_actions(model);
        let mut requirements = Vec::new();
        let mut action_requirements = Vec::new();
        let mut effects: Vec<Vec<EffectInfo>> = Vec::new();
        let mut statics = Vec::new();
        let mut empty_domain = Vec::new();
        for (i, a) in actions.iter().enumerate() {
            let scope = Scope {
                params: (a.parameters.iter())
                    .map(|p| (p.name().clone(), Rc::new(domains.domain(a, p.name(), p.tpe()))))
                    .collect(),
            };
            empty_domain.push(scope.params.values().any(|d| d.is_empty()));
            let mut reqs = Vec::new();
            let mut a_statics = Vec::new();
            for c in &a.conditions {
                for conj in conjuncts(c.cond, env) {
                    if let Some(req) = simple_requirement(conj, env) {
                        if !modified.contains(&req.fluent) {
                            a_statics.push(req.clone());
                        }
                        reqs.push(requirements.len());
                        requirements.push(Requirement {
                            owner: Owner::Action(i, c.id),
                            pattern: scope.requirement(&req),
                            req,
                        });
                    }
                }
            }
            action_requirements.push(reqs);
            statics.push(a_statics);
            let simple = |e: Option<ExprId>| e.and_then(|e| SimpleValue::of(e, env));
            effects.push(
                (a.effects.iter())
                    .map(|e| {
                        let t = a.transition_of_effect(e, env);
                        EffectInfo {
                            id: e.id,
                            pattern: effect_pattern(e, &scope, env),
                            borrow: t.is_some_and(|t| t.borrow),
                            transition: t
                                .filter(|t| !t.borrow)
                                .and_then(|t| Some((SimpleValue::required(t.required, env)?, simple(t.value)?))),
                        }
                    })
                    .collect(),
            );
        }
        // lock restores are equivalent to borrows if no one can change the state variable while it is locked
        let restores: Vec<Vec<bool>> = (actions.iter())
            .zip(&effects)
            .map(|(a, effs)| {
                let timing = ActionTiming::new(a, &bounds);
                (a.effects.iter())
                    .zip(effs)
                    .map(|(e, info): (&Effect, &EffectInfo)| !info.borrow && lock_restore(a, e, &timing, env))
                    .collect()
            })
            .collect();
        // values written by unguarded effects (that are not lock restores) on each fluent,
        // `None` standing for an erasure or a value that is not a constant
        let mut unguarded: HashMap<FluentId, Vec<Option<SimpleValue>>> = HashMap::new();
        for (a, rs) in actions.iter().zip(&restores) {
            for (e, &restore) in a.effects.iter().zip(rs) {
                if a.transition_of_effect(e, env).is_none() && !restore {
                    unguarded
                        .entry(e.effect_expression.state_variable.fluent)
                        .or_default()
                        .push(constant_value(e, env));
                }
            }
        }
        for e in &model.init {
            if e.effect_expression.transition_start != Timestamp::ORIGIN {
                unguarded
                    .entry(e.effect_expression.state_variable.fluent)
                    .or_default()
                    .push(None);
            }
        }
        for ((a, effs), rs) in actions.iter().zip(effects.iter_mut()).zip(&restores) {
            for ((e, info), &restore) in a.effects.iter().zip(effs.iter_mut()).zip(rs) {
                // safe if no unguarded effect may write another value while the state variable is erased
                let value = constant_value(e, env);
                let safe =
                    (unguarded.get(&info.pattern.fluent).into_iter().flatten()).all(|v| v.is_some() && *v == value);
                if restore && safe {
                    info.borrow = true;
                }
            }
        }

        for g in &model.goals {
            for req in simple_goal(g, env).unwrap_or_default() {
                requirements.push(Requirement {
                    owner: Owner::Goal(g.id),
                    pattern: Scope::default().requirement(&req),
                    req,
                });
            }
        }
        let mut init: Vec<Pattern> = (model.init.iter())
            .map(|e| effect_pattern(e, &Scope::default(), env))
            .collect();
        // closed world assumption: boolean fluents are initially false unless stated otherwise
        for (id, f) in env.fluents.iter_with_id() {
            if f.return_type == Type::Bool {
                init.push(Pattern {
                    fluent: id,
                    args: vec![Dom::Any; f.parameters.len()],
                    value: Some(Dom::Bool(false)),
                })
            }
        }
        let mut analysis = SupportAnalysis {
            model,
            timings: actions.iter().map(|a| ActionTiming::new(a, &bounds)).collect(),
            bounds,
            actions,
            effects,
            requirements,
            action_requirements,
            empty_domain,
            init,
            opaque: opaque_fluents(model),
            numeric: numeric_properties(model),
            statics,
            functions: StaticFunctions::new(model),
            useless: Vec::new(),
        };
        analysis.useless = (0..analysis.actions.len())
            .map(|i| {
                (0..analysis.effects[i].len())
                    .map(|j| analysis.targets(i, j, false).is_some_and(|ts| ts.is_empty()))
                    .collect()
            })
            .collect();
        analysis
    }

    /// Requirements that the effect (`j`-th of action `i`) may support, `None` if it is opaque.
    /// If `with_undo` is false, no target is flagged as an undo.
    fn targets(&self, i: usize, j: usize, with_undo: bool) -> Option<Vec<SupportTarget>> {
        let eff = &self.effects[i][j];
        let action = self.actions[i];
        let fluent = eff.pattern.fluent;
        if eff.borrow || eff.pattern.value.is_none() {
            return Some(Vec::new());
        }
        if self.opaque.contains(&fluent) {
            // numeric changes in a direction that never helps are useless
            if let Some(num) = self.numeric.get(&fluent) {
                let useless = |d: DirectionImpact| matches!(d, DirectionImpact::Harmful | DirectionImpact::Neutral);
                let nonnegative = |d: ExprId| self.bounds.bounds(d).lb.is_some_and(|lb| lb >= RealValue::ZERO);
                match action.effects[j].effect_expression.operation {
                    EffectOp::Increase(d) if nonnegative(d) && useless(num.increase) => return Some(Vec::new()),
                    EffectOp::Decrease(d) if nonnegative(d) && useless(num.decrease) => return Some(Vec::new()),
                    _ => {}
                }
            }
            return None;
        }
        let mut targets = Vec::new();
        let mut goals: Vec<(GoalId, usize)> = Vec::new();
        for r in &self.requirements {
            if !eff.pattern.may_support(&r.pattern) {
                continue;
            }
            match &r.owner {
                Owner::Action(k, cond) => {
                    let target = SupportTarget::Condition {
                        cond: CondRef {
                            action: self.actions[*k].name.clone(),
                            condition: *cond,
                        },
                        undo: with_undo && self.is_undo(i, j, *k, &r.req),
                    };
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                }
                Owner::Goal(g) => match goals.iter_mut().find(|(g2, _)| g2 == g) {
                    Some((_, n)) => *n += 1,
                    None => goals.push((*g, 1)),
                },
            }
        }
        targets.extend(
            goals
                .into_iter()
                .map(|(goal, atoms)| SupportTarget::Goal { goal, atoms }),
        );
        Some(targets)
    }

    /// True if the requirement `req` of action `k` is the condition of a transition that, when supported by the
    /// effect (`j`-th of action `i`), restores the value consumed by the effect's transition, and if action `k` has no
    /// other useful effect: the two actions then cancel each other.
    fn is_undo(&self, i: usize, j: usize, k: usize, req: &SimpleRequirement) -> bool {
        let env = &self.model.env;
        let Some((pre_a, post_a)) = &self.effects[i][j].transition else {
            return false;
        };
        // the transition of `k` whose condition is `req`
        let b = self.actions[k];
        let Some((t_b, post_b)) = (b.transitions(env))
            .filter(|t| !t.borrow)
            .filter(|t| simple_requirement(t.condition.cond, env).as_ref() == Some(req))
            .find_map(|t| Some((t, SimpleValue::of(t.value?, env)?)))
        else {
            return false;
        };
        // all other effects of `k` must be useless
        let others_useless = (0..b.effects.len())
            .filter(|&j2| b.effects[j2].id != t_b.effect.id)
            .all(|j2| self.useless[k][j2]);
        if !others_useless {
            return false;
        }
        let a_args = &self.actions[i].effects[j].effect_expression.state_variable.arguments;
        let Some(a_args) = Term::all_of(a_args, env) else {
            return false;
        };
        let mut uf = UnionFind::default();
        // the effect supports the requirement: same state variable and value
        for (x, y) in a_args.iter().zip(&req.args) {
            uf.union(Key::term(Side::A, x), Key::term(Side::B, y));
        }
        uf.union(Key::value(Side::A, post_a), Key::value(Side::B, &req.value));
        // propagate equalities through static functions required by both actions
        loop {
            let mut changed = false;
            for ra in &self.statics[i] {
                for rb in &self.statics[k] {
                    if ra.fluent != rb.fluent || ra.args.len() != rb.args.len() {
                        continue;
                    }
                    let same = |uf: &mut UnionFind, x: &Term, y: &Term| {
                        uf.find(Key::term(Side::A, x)) == uf.find(Key::term(Side::B, y))
                    };
                    let (ka, kb) = (Key::value(Side::A, &ra.value), Key::value(Side::B, &rb.value));
                    if !matches!(ra.value, SimpleValue::Bool(_)) {
                        // `f(args) = v`: equal arguments imply equal values
                        if ra.args.iter().zip(&rb.args).all(|(x, y)| same(&mut uf, x, y))
                            && uf.find(ka.clone()) != uf.find(kb.clone())
                        {
                            uf.union(ka, kb);
                            changed = true;
                        }
                    } else if ra.value == SimpleValue::Bool(true)
                        && rb.value == SimpleValue::Bool(true)
                        && ra.args.len() == 2
                    {
                        // functional binary relation: equal inputs imply equal outputs
                        for input in [0, 1] {
                            let f = StaticFunction {
                                fluent: ra.fluent,
                                input,
                            };
                            let out = f.output();
                            if self.functions.is_function(&f)
                                && same(&mut uf, &ra.args[input], &rb.args[input])
                                && !same(&mut uf, &ra.args[out], &rb.args[out])
                            {
                                uf.union(Key::term(Side::A, &ra.args[out]), Key::term(Side::B, &rb.args[out]));
                                changed = true;
                            }
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        uf.find(Key::value(Side::B, &post_b)) == uf.find(Key::value(Side::A, pre_a))
    }

    /// True if the requirement may be supported by the problem definition.
    fn supported_by_init(&self, r: &Requirement) -> bool {
        self.init.iter().any(|p| p.may_support(&r.pattern))
    }

    /// Computes the annotations of each action, and the supporters of the requirements of each goal.
    fn run(&self) -> (Vec<(ActionRef, ActionResult)>, GoalSupporters) {
        let env = &self.model.env;
        let n = self.actions.len();
        // supports of each effect
        let supports: Vec<Vec<Option<Vec<SupportTarget>>>> = (0..n)
            .map(|i| (0..self.effects[i].len()).map(|j| self.targets(i, j, true)).collect())
            .collect();

        // potential supporters of each requirement (ignoring opacity, but excluding borrows and erasures)
        let supporters: Vec<Vec<(usize, usize)>> = (self.requirements.iter())
            .map(|r| {
                let mut res = Vec::new();
                for (i, effs) in self.effects.iter().enumerate() {
                    for (j, e) in effs.iter().enumerate() {
                        if !e.borrow && e.pattern.may_support(&r.pattern) {
                            res.push((i, j));
                        }
                    }
                }
                res
            })
            .collect();
        let init: Vec<bool> = self.requirements.iter().map(|r| self.supported_by_init(r)).collect();
        let eff_ref = |(k, j): (usize, usize)| EffRef {
            action: self.actions[k].name.clone(),
            effect: self.effects[k][j].id,
        };
        let mut goals: HashMap<GoalId, Vec<Supporters>> = HashMap::new();
        for (r, req) in self.requirements.iter().enumerate() {
            if let Owner::Goal(g) = req.owner {
                goals.entry(g).or_default().push(Supporters {
                    init: init[r],
                    effects: supporters[r].iter().map(|&s| eff_ref(s)).collect(),
                });
            }
        }

        // reachability (least fixpoint)
        let mut reachable = vec![false; n];
        loop {
            let mut changed = false;
            for i in 0..n {
                if reachable[i] || self.empty_domain[i] {
                    continue;
                }
                let action = self.actions[i];
                let ok = self.action_requirements[i].iter().all(|&r| {
                    init[r]
                        || supporters[r].iter().any(|&(k, j)| {
                            reachable[k]
                                || (k == i && {
                                    // an effect of the same instance occurring before the condition
                                    let Owner::Action(_, cid) = self.requirements[r].owner else {
                                        unreachable!()
                                    };
                                    let cond = action.condition(cid).unwrap();
                                    let eff = &action.effects[j].effect_expression;
                                    self.timings[i].le(eff.transition_end, cond.interval.start)
                                })
                        })
                });
                if ok {
                    reachable[i] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // usefulness (least fixpoint)
        let hierarchical = self.model.task_network.is_some() || self.actions.iter().any(|a| !a.subtasks.is_empty());
        let index: HashMap<&ActionRef, usize> = self.actions.iter().enumerate().map(|(i, a)| (&a.name, i)).collect();
        let mut useful = vec![false; n];
        loop {
            let mut changed = false;
            for i in 0..n {
                if useful[i] || !reachable[i] {
                    continue;
                }
                let ok = hierarchical
                    || supports[i].iter().any(|s| match s {
                        None => true,
                        Some(targets) => targets.iter().any(|t| match t {
                            SupportTarget::Goal { .. } => true,
                            SupportTarget::Condition { cond, .. } => useful[index[&cond.action]],
                        }),
                    });
                if ok {
                    useful[i] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        let mut results = Vec::new();
        for i in 0..n {
            let action = self.actions[i];
            let mut cond_supporters = HashMap::new();
            for c in &action.conditions {
                let reqs = (self.action_requirements[i].iter())
                    .filter(|&&r| matches!(self.requirements[r].owner, Owner::Action(_, cid) if cid == c.id))
                    .copied()
                    .collect::<Vec<usize>>();
                // only conditions made of a single simple requirement are annotated
                if let &[r] = reqs.as_slice()
                    && conjuncts(c.cond, env).len() == 1
                {
                    let effects = supporters[r].iter().map(|&s| eff_ref(s)).collect();
                    cond_supporters.insert(c.id, Supporters { init: init[r], effects });
                }
            }
            let effect_supports = (self.effects[i].iter().zip(&supports[i]))
                .map(|(e, s)| {
                    let s = match s {
                        None => Supports::Opaque,
                        Some(targets) => Supports::Targets(targets.clone()),
                    };
                    (e.id, s)
                })
                .collect();
            results.push((
                action.name.clone(),
                ActionResult {
                    status: ActionStatus {
                        reachable: reachable[i],
                        useful: useful[i],
                    },
                    supporters: cond_supporters,
                    supports: effect_supports,
                },
            ));
        }
        (results, goals)
    }
}

/// Side of a pair of actions whose parameters are related by the union-find.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum Side {
    A,
    B,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Param(Side, Sym),
    Object(Sym),
    Bool(bool),
    Real(RealValue),
}

impl Key {
    fn term(side: Side, t: &Term) -> Key {
        match t {
            Term::Param(p) => Key::Param(side, p.clone()),
            Term::Object(o) => Key::Object(o.clone()),
        }
    }
    fn value(side: Side, v: &SimpleValue) -> Key {
        match v {
            SimpleValue::Bool(b) => Key::Bool(*b),
            SimpleValue::Real(r) => Key::Real(*r),
            SimpleValue::Term(t) => Key::term(side, t),
        }
    }
}

#[derive(Default)]
struct UnionFind {
    parent: HashMap<Key, Key>,
}

impl UnionFind {
    fn find(&mut self, k: Key) -> Key {
        match self.parent.get(&k).cloned() {
            Some(p) if p != k => {
                let root = self.find(p);
                self.parent.insert(k, root.clone());
                root
            }
            _ => k,
        }
    }
    fn union(&mut self, a: Key, b: Key) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent.insert(a, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::pddl::{build_model, input::Input, parser};
    use crate::preprocessing::{ModelTransformation, apply_transformations};
    use crate::*;

    fn build(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let model = build_model(&domain, &problem).unwrap();
        use ModelTransformation::*;
        let model = apply_transformations(
            model,
            &[
                EraseUnusableEffects,
                SplitPredicates,
                LiftPredicates,
                MergeConditions,
                MergeEffectCondition,
                MergeEraseAssign,
                AnnotateTransitions,
            ],
        )
        .unwrap();
        let model = apply_transformations(model, &[AnnotateSupports]).unwrap();
        println!("{model}");
        model
    }

    fn action<'a>(model: &'a Model, name: &str) -> &'a Action {
        model.actions.get_action(&Sym::from(name)).unwrap()
    }

    fn status(model: &Model, name: &str) -> (bool, bool) {
        let s = action(model, name).annotations.status.unwrap();
        (s.reachable, s.useful)
    }

    /// Supports of the effects of an action on a fluent, as displayed.
    fn supports(model: &Model, name: &str, fluent: &str) -> Vec<String> {
        let a = action(model, name);
        let fluent = model.env.fluents.get_by_name(fluent).unwrap();
        (a.effects.iter())
            .filter(|e| e.effect_expression.state_variable.fluent == fluent)
            .map(|e| {
                e.annotations
                    .supports
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .split(", ")
                    .map(|t| {
                        t.split('.')
                            .next()
                            .unwrap()
                            .trim_end_matches(|c: char| c.is_ascii_digit())
                            .to_string()
                            + if t.ends_with("(undo)") { "(undo)" } else { "" }
                    })
                    .join(", ")
            })
            .collect()
    }

    const DOMAIN: &str = "(define (domain d) (:requirements :typing :durative-actions)
        (:types sat inst dir)
        (:predicates (on_board ?i - inst ?s - sat) (power_avail ?s - sat) (power_on ?i - inst)
                     (calibrated ?i - inst) (pointing ?s - sat ?d - dir) (have_image ?d - dir)
                     (free ?s - sat) (logged ?d - dir) (never ?d - dir) (unused ?d - dir))
        (:action switch_on :parameters (?i - inst ?s - sat)
          :precondition (and (on_board ?i ?s) (power_avail ?s))
          :effect (and (power_on ?i) (not (calibrated ?i)) (not (power_avail ?s))))
        (:action switch_off :parameters (?i - inst ?s - sat)
          :precondition (and (on_board ?i ?s) (power_on ?i))
          :effect (and (not (power_on ?i)) (power_avail ?s)))
        (:action calibrate :parameters (?s - sat ?i - inst)
          :precondition (and (on_board ?i ?s) (power_on ?i))
          :effect (calibrated ?i))
        (:durative-action take_image :parameters (?s - sat ?d - dir ?i - inst) :duration (= ?duration 2)
          :condition (and (at start (calibrated ?i)) (over all (on_board ?i ?s)) (over all (power_on ?i))
                          (over all (pointing ?s ?d)) (at start (free ?s)))
          :effect (and (at end (have_image ?d)) (at start (not (free ?s))) (at end (free ?s))))
        (:action turn_to :parameters (?s - sat ?to - dir ?from - dir)
          :precondition (pointing ?s ?from)
          :effect (and (pointing ?s ?to) (not (pointing ?s ?from))))
        (:action log :parameters (?d - dir) :precondition (have_image ?d) :effect (logged ?d))
        (:action impossible :parameters (?d - dir) :precondition (never ?d) :effect (have_image ?d)))";

    const PROBLEM: &str = "(define (problem p) (:domain d)
        (:objects s1 s2 - sat i1 i2 i3 - inst d1 d2 d3 - dir)
        (:init (on_board i1 s1) (on_board i2 s1) (on_board i3 s2) (power_avail s1) (power_avail s2)
               (pointing s1 d1) (pointing s2 d1) (free s1) (free s2))
        (:goal (and (have_image d2) (have_image d3))))";

    #[test]
    fn supports_and_status() {
        let model = build(DOMAIN, PROBLEM);
        // goals: one atom each (and a condition of the useless `log`)
        assert_eq!(supports(&model, "take_image", "have_image"), ["log, g, g"]);
        // switch_on restores what switch_off consumed, but not the other way around
        assert_eq!(
            supports(&model, "switch_on", "power_avail:power_on"),
            ["calibrate, switch_off(undo), take_image"]
        );
        assert_eq!(supports(&model, "switch_off", "power_avail:power_on"), ["switch_on"]);
        // erasure (unusable delete) and borrow support nothing
        assert_eq!(supports(&model, "switch_on", "calibrated"), ["<nothing>"]);
        assert_eq!(supports(&model, "take_image", "free"), ["<nothing>"]);
        assert_eq!(supports(&model, "calibrate", "calibrated"), ["take_image"]);
        // `logged` is never read
        assert_eq!(supports(&model, "log", "logged"), ["<nothing>"]);

        for a in ["switch_on", "switch_off", "calibrate", "take_image", "turn_to"] {
            assert_eq!(status(&model, a), (true, true), "{a}");
        }
        assert_eq!(status(&model, "log"), (true, false));
        assert_eq!(status(&model, "impossible"), (false, false));

        // supporters of conditions
        let take = action(&model, "take_image");
        let sup = |name: &str| {
            let fluent = model.env.fluents.get_by_name(name).unwrap();
            let c = (take.conditions.iter())
                .find(|c| {
                    crate::preprocessing::analysis::simple_requirement(c.cond, &model.env)
                        .is_some_and(|r| r.fluent == fluent)
                })
                .unwrap();
            let s = c.annotations.supporters.as_ref().unwrap().to_string();
            s.split(", ")
                .map(|t| t.split('.').next().unwrap().to_string())
                .dedup()
                .join(", ")
        };
        assert_eq!(sup("calibrated"), "calibrate");
        assert_eq!(sup("pointing"), "init, turn_to");
        assert_eq!(sup("on_board"), "init");
    }
}
