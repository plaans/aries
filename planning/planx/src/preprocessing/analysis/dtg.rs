//! Domain transition graphs (DTG) of the fluents of a model.
//!
//! The DTG of a fluent `f` is a graph whose nodes are the values that the state variables of `f` may take, and whose
//! edges are the transitions (see [`annotate_transitions`](crate::preprocessing::annotate_transitions)) of the
//! actions on `f`.
//!
//! The graph is *value-grounded* but *argument-lifted*: the edges of a transition `f(args) : pre =~> post` are all
//! pairs of values that `pre` and `post` may take, restricted by the static requirements of the action relating
//! them (e.g. `can_traverse(?r, ?from, ?to)`), but they are not distinguished by the arguments of the state
//! variable. The graph is thus the *union* of the graphs of all state variables of `f`. Since the edges are
//! over-approximated (each static requirement is projected independently), the graph is a super-graph of the one of
//! each state variable: the set of reachable values, the absence of cycles and the length of the longest path can be
//! soundly used to bound the transitions of each state variable.
//!
//! Effects on `f` that are not part of a transition are *writers*: they may set the state variable to their value
//! whatever its previous value.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Display;

use itertools::Itertools;

use crate::*;

use super::{
    ParamDomains, SimpleRequirement, SimpleValue, Term, conjuncts, fluents_modified_by_actions, simple_goal,
    simple_requirement,
};

/// A value of a state variable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DtgValue {
    Bool(bool),
    Object(Sym),
    /// No value (erased state variable).
    Undefined,
}

impl Display for DtgValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DtgValue::Bool(b) => write!(f, "{b}"),
            DtgValue::Object(o) => write!(f, "{o}"),
            DtgValue::Undefined => write!(f, "∅"),
        }
    }
}

/// The transitions of an action on the fluent of a DTG.
#[derive(Clone, Debug)]
pub struct DtgTransitions {
    pub action: ActionRef,
    /// Number of transitions on the fluent in each instance of the action.
    pub count: usize,
    /// Possible (source, target) pairs of values (indices in [`Dtg::values`]), over all transitions of the action.
    pub edges: BTreeSet<(usize, usize)>,
}

/// An effect on the fluent that is not part of a transition.
#[derive(Clone, Debug)]
pub struct DtgWriter {
    pub action: ActionRef,
    /// Possible values written (indices in [`Dtg::values`]).
    pub values: BTreeSet<usize>,
}

/// Domain transition graph of a fluent (see the module documentation).
#[derive(Clone, Debug)]
pub struct Dtg {
    pub fluent: FluentId,
    /// All possible values (the last one is [`DtgValue::Undefined`]).
    pub values: Vec<DtgValue>,
    /// Transitions, grouped by action.
    pub transitions: Vec<DtgTransitions>,
    /// Effects on the fluent that are not transitions.
    pub writers: Vec<DtgWriter>,
    /// True if the fluent is modified by effects of the problem definition after the initial state.
    pub timed_effects: bool,
    /// Number of state variables of the fluent (product of the number of objects of each argument type).
    pub num_state_variables: u64,
    /// For each value, the number of state variables that initially have this value.
    pub initial: Vec<u64>,
    /// For each value, the number of distinct goal atoms requiring a state variable to have this value.
    pub goals: Vec<u64>,
}

impl Dtg {
    /// Builds the DTG of all non-static fluents whose values are booleans or objects.
    /// Fluents with too many values (more than `max_values`) are ignored.
    pub fn all(model: &Model, max_values: usize) -> Vec<Dtg> {
        let modified = fluents_modified_by_actions(model);
        let domains = ParamDomains::new(model);
        let statics = StaticTables::new(model, &modified);
        let mut ids = model.env.fluents.iter_with_id().map(|(id, _)| id).collect_vec();
        ids.sort();
        (ids.into_iter())
            .filter(|id| modified.contains(id))
            .filter_map(|id| Dtg::new(model, id, &domains, &statics, max_values))
            .collect()
    }

    fn new(
        model: &Model,
        fluent: FluentId,
        domains: &ParamDomains,
        statics: &StaticTables,
        max_values: usize,
    ) -> Option<Dtg> {
        let env = &model.env;
        let f = env.fluents.get(fluent);
        let mut values = match &f.return_type {
            Type::Bool => vec![DtgValue::Bool(false), DtgValue::Bool(true)],
            tpe @ Type::User(_) => objects_of(tpe, env)
                .into_iter()
                .sorted()
                .map(DtgValue::Object)
                .collect(),
            _ => return None,
        };
        if values.len() > max_values {
            return None;
        }
        values.push(DtgValue::Undefined);
        let index: HashMap<DtgValue, usize> = values.iter().cloned().enumerate().map(|(i, v)| (v, i)).collect();
        let all: BTreeSet<usize> = (0..values.len() - 1).collect();
        let dom = |action: &Action, v: &SimpleValue| -> BTreeSet<usize> {
            match v {
                SimpleValue::Bool(b) => index.get(&DtgValue::Bool(*b)).into_iter().copied().collect(),
                SimpleValue::Term(Term::Object(o)) => {
                    index.get(&DtgValue::Object(o.clone())).into_iter().copied().collect()
                }
                SimpleValue::Term(Term::Param(p)) => match action.parameters.iter().find(|x| x.name() == p) {
                    Some(param) => (domains.domain(action, p, param.tpe()).into_iter())
                        .filter_map(|o| index.get(&DtgValue::Object(o)).copied())
                        .collect(),
                    None => all.clone(),
                },
                SimpleValue::Real(_) => BTreeSet::new(),
            }
        };

        let mut transitions = Vec::new();
        let mut writers = Vec::new();
        for a in model.actions.iter() {
            let mut edges = BTreeSet::new();
            let mut count = 0;
            let mut values_written = BTreeSet::new();
            for e in &a.effects {
                if e.effect_expression.state_variable.fluent != fluent {
                    continue;
                }
                let value = match e.effect_expression.operation {
                    EffectOp::Assign(v) => SimpleValue::of(v, env),
                    EffectOp::Erase => None,
                    EffectOp::Increase(_) | EffectOp::Decrease(_) => return None,
                };
                let post: BTreeSet<usize> = match (&e.effect_expression.operation, &value) {
                    (EffectOp::Erase, _) => [values.len() - 1].into(),
                    (_, Some(v)) => dom(a, v),
                    (_, None) => all.clone(),
                };
                match a.transition_of_effect(e, env) {
                    Some(t) if !t.borrow => {
                        count += 1;
                        let pre_value = SimpleValue::required(t.required, env);
                        let pre = pre_value.as_ref().map_or_else(|| all.clone(), |v| dom(a, v));
                        let allowed = PairFilter::new(a, pre_value.as_ref(), value.as_ref(), env, statics, &index);
                        for &u in &pre {
                            for &w in &post {
                                if allowed.accepts(u, w) {
                                    edges.insert((u, w));
                                }
                            }
                        }
                    }
                    Some(_) => {} // borrow: no change
                    None => values_written.extend(post),
                }
            }
            if count > 0 {
                transitions.push(DtgTransitions {
                    action: a.name.clone(),
                    count,
                    edges,
                });
            }
            if !values_written.is_empty() {
                writers.push(DtgWriter {
                    action: a.name.clone(),
                    values: values_written,
                });
            }
        }

        // initial values (booleans are false unless stated otherwise)
        let mut timed_effects = false;
        let mut initial_svs: HashMap<Vec<Term>, usize> = HashMap::new();
        for e in &model.init {
            let ee = &e.effect_expression;
            if ee.state_variable.fluent != fluent {
                continue;
            }
            if ee.transition_start != Timestamp::ORIGIN {
                timed_effects = true;
                continue;
            }
            let args = Term::all_of(&ee.state_variable.arguments, env)?;
            let v = match ee.operation {
                EffectOp::Assign(v) => SimpleValue::of(v, env)?,
                _ => return None,
            };
            let v = match v {
                SimpleValue::Bool(b) => DtgValue::Bool(b),
                SimpleValue::Term(Term::Object(o)) => DtgValue::Object(o),
                _ => return None,
            };
            initial_svs.insert(args, *index.get(&v)?);
        }
        let num_state_variables = (f.parameters.iter())
            .map(|p| objects_of(p.tpe(), env).len() as u64)
            .fold(1u64, |acc, n| acc.saturating_mul(n));
        let mut initial = vec![0; values.len()];
        for v in initial_svs.values() {
            initial[*v] += 1;
        }
        if f.return_type == Type::Bool {
            initial[index[&DtgValue::Bool(false)]] += num_state_variables.saturating_sub(initial_svs.len() as u64);
        } else {
            initial[values.len() - 1] += num_state_variables.saturating_sub(initial_svs.len() as u64);
        }

        // distinct goal atoms
        let mut goal_atoms = HashSet::new();
        for g in &model.goals {
            for r in simple_goal(g, env).unwrap_or_default() {
                if r.fluent == fluent
                    && let Some(&v) = dom_of_constant(&r, &index)
                {
                    goal_atoms.insert((r.args.clone(), v));
                }
            }
        }
        let mut goals = vec![0; values.len()];
        for (_, v) in goal_atoms {
            goals[v] += 1;
        }

        Some(Dtg {
            fluent,
            values,
            transitions,
            writers,
            timed_effects,
            num_state_variables,
            initial,
            goals,
        })
    }

    /// Values that may be taken by some state variable: initial values, values set by writers and values reachable
    /// from them through transitions.
    pub fn reachable(&self) -> BTreeSet<usize> {
        let mut reached: BTreeSet<usize> = (0..self.values.len()).filter(|&v| self.initial[v] > 0).collect();
        reached.extend(self.writers.iter().flat_map(|w| w.values.iter().copied()));
        let mut succs: HashMap<usize, Vec<usize>> = HashMap::new();
        for t in &self.transitions {
            for &(u, w) in &t.edges {
                succs.entry(u).or_default().push(w);
            }
        }
        let mut stack = reached.iter().copied().collect_vec();
        while let Some(u) = stack.pop() {
            for &w in succs.get(&u).into_iter().flatten() {
                if reached.insert(w) {
                    stack.push(w);
                }
            }
        }
        reached
    }

    /// Edges between reachable values.
    fn reachable_edges(&self) -> BTreeSet<(usize, usize)> {
        let reachable = self.reachable();
        (self.transitions.iter())
            .flat_map(|t| t.edges.iter().copied())
            .filter(|(u, _)| reachable.contains(u))
            .collect()
    }

    /// If the graph restricted to reachable values has no cycle (including self-loops), the number of edges of its
    /// longest path. Each state variable can then perform at most this number of transitions.
    pub fn longest_path(&self) -> Option<u64> {
        let edges = self.reachable_edges();
        let mut succs: HashMap<usize, Vec<usize>> = HashMap::new();
        for (u, w) in edges {
            succs.entry(u).or_default().push(w);
        }
        fn visit(
            n: usize,
            succs: &HashMap<usize, Vec<usize>>,
            done: &mut HashMap<usize, u64>,
            active: &mut HashSet<usize>,
        ) -> Option<u64> {
            if let Some(&l) = done.get(&n) {
                return Some(l);
            }
            if !active.insert(n) {
                return None;
            }
            let mut l = 0;
            for &s in succs.get(&n).into_iter().flatten() {
                l = l.max(1 + visit(s, succs, done, active)?);
            }
            active.remove(&n);
            done.insert(n, l);
            Some(l)
        }
        let mut done = HashMap::new();
        let mut active = HashSet::new();
        let mut longest = 0;
        for &n in succs.keys() {
            longest = longest.max(visit(n, &succs, &mut done, &mut active)?);
        }
        Some(longest)
    }

    /// Upper bound on the number of edges of a path that visits each value at most once: the number of reachable
    /// values minus one, or the longest path if the graph is acyclic.
    pub fn max_simple_path(&self) -> u64 {
        let n = (self.reachable().len() as u64).saturating_sub(1);
        self.longest_path().map_or(n, |l| l.min(n))
    }
}

/// The value index of a requirement on a constant value.
fn dom_of_constant<'a>(r: &SimpleRequirement, index: &'a HashMap<DtgValue, usize>) -> Option<&'a usize> {
    match &r.value {
        SimpleValue::Bool(b) => index.get(&DtgValue::Bool(*b)),
        SimpleValue::Term(Term::Object(o)) => index.get(&DtgValue::Object(o.clone())),
        _ => None,
    }
}

fn objects_of(tpe: &Type, env: &Environment) -> Vec<Sym> {
    (env.objects.iter())
        .filter(|o| Type::from(o.tpe()).is_subtype_of(tpe))
        .map(|o| o.name().clone())
        .collect()
}

impl Display for Env<'_, &Dtg> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dtg = self.elem;
        let name = (self.env / dtg.fluent).name();
        let reachable = dtg.reachable();
        write!(
            f,
            "{name}: {} values ({} reachable), {} state variables",
            dtg.values.len(),
            reachable.len(),
            dtg.num_state_variables
        )?;
        match dtg.longest_path() {
            Some(l) => write!(f, ", acyclic (longest path: {l})")?,
            None => write!(f, ", cyclic")?,
        }
        let vals = |vs: &mut dyn Iterator<Item = usize>| vs.map(|v| dtg.values[v].to_string()).join(", ");
        let init = (0..dtg.values.len()).filter(|&v| dtg.initial[v] > 0);
        write!(
            f,
            "\n    initial: {}",
            init.map(|v| format!("{}x{}", dtg.values[v], dtg.initial[v])).join(", ")
        )?;
        let goals = (0..dtg.values.len()).filter(|&v| dtg.goals[v] > 0).collect_vec();
        if !goals.is_empty() {
            write!(
                f,
                "\n    goals: {}",
                goals
                    .iter()
                    .map(|&v| format!("{}x{}", dtg.values[v], dtg.goals[v]))
                    .join(", ")
            )?;
        }
        for t in &dtg.transitions {
            let shown = t
                .edges
                .iter()
                .take(8)
                .map(|(u, w)| format!("{}->{}", dtg.values[*u], dtg.values[*w]));
            let more = if t.edges.len() > 8 {
                format!(", ... ({} edges)", t.edges.len())
            } else {
                String::new()
            };
            write!(
                f,
                "\n    transitions of {} (x{}): {}{more}",
                t.action,
                t.count,
                shown.into_iter().join(", ")
            )?;
        }
        for w in &dtg.writers {
            write!(
                f,
                "\n    writer {}: * -> {}",
                w.action,
                vals(&mut w.values.iter().copied())
            )?;
        }
        if dtg.timed_effects {
            write!(f, "\n    timed effects in the problem definition")?;
        }
        Ok(())
    }
}

/// A row of a static table: arguments and value.
type Row = (Vec<Sym>, SimpleValue);

/// Rows of the static fluents: arguments and value of each initial assignment.
struct StaticTables {
    rows: HashMap<FluentId, Vec<Row>>,
}

impl StaticTables {
    fn new(model: &Model, modified: &HashSet<FluentId>) -> Self {
        let env = &model.env;
        let mut rows: HashMap<FluentId, Option<Vec<Row>>> = HashMap::new();
        for e in &model.init {
            let ee = &e.effect_expression;
            let fluent = ee.state_variable.fluent;
            if modified.contains(&fluent) {
                continue;
            }
            let row = (|| {
                if ee.condition.is_some() || !e.universal_quantification.is_empty() {
                    return None;
                }
                let args = (ee.state_variable.arguments.iter())
                    .map(|&a| match Term::of(a, env)? {
                        Term::Object(o) => Some(o),
                        Term::Param(_) => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                let EffectOp::Assign(v) = ee.operation else { return None };
                Some((args, SimpleValue::of(v, env)?))
            })();
            let entry = rows.entry(fluent).or_insert(Some(Vec::new()));
            match (row, entry.as_mut()) {
                (Some(row), Some(rs)) => rs.push(row),
                _ => *entry = None,
            }
        }
        StaticTables {
            rows: rows.into_iter().filter_map(|(f, rs)| Some((f, rs?))).collect(),
        }
    }
}

/// Restriction of the pairs (source, target) of a transition imposed by the static requirements of its action.
struct PairFilter {
    /// For each static requirement relating the source and/or the target, the allowed (source, target) values
    /// (`None` when the requirement does not constrain one of them).
    allowed: Vec<HashSet<(Option<usize>, Option<usize>)>>,
    /// Patterns of the allowed sets: (constrains the source, constrains the target).
    shapes: Vec<(bool, bool)>,
    /// True if the source and target are required to be distinct.
    distinct: bool,
}

impl PairFilter {
    fn new(
        action: &Action,
        pre: Option<&SimpleValue>,
        post: Option<&SimpleValue>,
        env: &Environment,
        statics: &StaticTables,
        index: &HashMap<DtgValue, usize>,
    ) -> Self {
        let param = |v: Option<&SimpleValue>| match v {
            Some(SimpleValue::Term(Term::Param(p))) => Some(p.clone()),
            _ => None,
        };
        let (p, q) = (param(pre), param(post));
        let mut allowed = Vec::new();
        let mut shapes = Vec::new();
        let mut distinct = false;
        for c in &action.conditions {
            for conj in conjuncts(c.cond, env) {
                // `(not (= ?p ?q))`
                if let Expr::App(Fun::Not, args) = env.node(conj).expr()
                    && let [e] = args.as_slice()
                    && let Expr::App(Fun::Eq, xs) = env.node(*e).expr()
                    && let [x, y] = xs.as_slice()
                {
                    let (x, y) = (Term::of(*x, env), Term::of(*y, env));
                    let is = |t: &Option<Term>, p: &Option<Sym>| {
                        p.is_some() && t.as_ref() == p.clone().map(Term::Param).as_ref()
                    };
                    if (is(&x, &p) && is(&y, &q)) || (is(&x, &q) && is(&y, &p)) {
                        distinct = true;
                    }
                    continue;
                }
                let Some(r) = simple_requirement(conj, env) else {
                    continue;
                };
                let Some(rows) = statics.rows.get(&r.fluent) else {
                    continue;
                };
                if r.value == SimpleValue::Bool(false) {
                    continue;
                }
                // terms of the requirement: arguments then value
                let mut terms: Vec<Option<Term>> = r.args.iter().cloned().map(Some).collect();
                terms.push(match &r.value {
                    SimpleValue::Term(t) => Some(t.clone()),
                    _ => None,
                });
                let is_param =
                    |t: &Option<Term>, p: &Option<Sym>| matches!((t, p), (Some(Term::Param(x)), Some(y)) if x == y);
                let (has_p, has_q) = (
                    terms.iter().any(|t| is_param(t, &p)),
                    terms.iter().any(|t| is_param(t, &q)),
                );
                if !has_p && !has_q {
                    continue;
                }
                let mut set = HashSet::new();
                'rows: for (args, value) in rows {
                    let mut row_terms: Vec<Option<Sym>> = args.iter().cloned().map(Some).collect();
                    row_terms.push(match value {
                        SimpleValue::Term(Term::Object(o)) => Some(o.clone()),
                        _ => None,
                    });
                    if let (SimpleValue::Bool(b), SimpleValue::Bool(rb)) = (&r.value, value)
                        && b != rb
                    {
                        continue;
                    }
                    let (mut pv, mut qv) = (None, None);
                    for (t, x) in terms.iter().zip(&row_terms) {
                        match t {
                            Some(Term::Object(o)) if x.as_ref() != Some(o) => continue 'rows,
                            _ if is_param(t, &p) => {
                                pv = x.clone().and_then(|o| index.get(&DtgValue::Object(o)).copied())
                            }
                            _ if is_param(t, &q) => {
                                qv = x.clone().and_then(|o| index.get(&DtgValue::Object(o)).copied())
                            }
                            _ => {}
                        }
                    }
                    set.insert((pv, qv));
                }
                allowed.push(set);
                shapes.push((has_p, has_q));
            }
        }
        if p.is_some() && p == q {
            // same parameter for the source and the target: only self-loops
            allowed.push(HashSet::new());
            shapes.push((false, false));
        }
        PairFilter {
            allowed,
            shapes,
            distinct,
        }
    }

    fn accepts(&self, u: usize, w: usize) -> bool {
        if self.distinct && u == w {
            return false;
        }
        self.allowed
            .iter()
            .zip(&self.shapes)
            .all(|(set, &(has_p, has_q))| match (has_p, has_q) {
                (true, true) => set.contains(&(Some(u), Some(w))),
                (true, false) => set.contains(&(Some(u), None)),
                (false, true) => set.contains(&(None, Some(w))),
                (false, false) => u == w,
            })
    }
}
