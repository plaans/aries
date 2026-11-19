//! Synthesis of groups of predicates that can be represented by a single state function.
//!
//! See the documentation of the parent module for the conditions checked and why they are sufficient.

use std::collections::{HashMap, HashSet, VecDeque};

use itertools::Itertools;

use crate::preprocessing::analysis::*;
use crate::*;

/// Maximum number of predicates in a group. Bounds the search to a polynomial number of candidates.
const MAX_GROUP_SIZE: usize = 4;

/// A component of the key of an atom.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyArg {
    /// The i-th argument of the atom.
    Arg(usize),
    /// The image of the i-th argument of the atom by a static function (e.g. the satellite of an instrument).
    Fun(StaticFunction, usize),
}

/// A predicate of a group, with the role of each of its parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Member {
    pub fluent: FluentId,
    /// Components of the key, in the group's key order.
    pub key: Vec<KeyArg>,
    /// Position of the argument that is the value of the atom.
    /// If `None`, the predicate is represented by a dedicated helper value.
    pub value: Option<usize>,
}

impl Member {
    /// Returns the key of the atom with the given arguments, `resolve` giving the value of static functions.
    pub fn key_of<T: Clone>(
        &self,
        args: &[T],
        mut resolve: impl FnMut(&StaticFunction, &T) -> Option<T>,
    ) -> Option<Vec<T>> {
        (self.key.iter())
            .map(|k| match k {
                KeyArg::Arg(i) => Some(args[*i].clone()),
                KeyArg::Fun(f, i) => resolve(f, &args[*i]),
            })
            .collect()
    }
    pub fn value_of<T: Clone>(&self, args: &[T]) -> Option<T> {
        self.value.map(|i| args[i].clone())
    }
}

/// A set of predicates such that, for any key, at most one atom of the group is true at any time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Group {
    /// Members of the group, in a canonical form (sorted by fluent id, with keys in the order of the first member)
    pub members: Vec<Member>,
}

impl Group {
    fn new(mut members: Vec<Member>) -> Self {
        members.sort_by_key(|m| m.fluent);
        let mut order: Vec<usize> = (0..members[0].key.len()).collect();
        order.sort_by_key(|&i| members[0].key[i]);
        for m in &mut members {
            m.key = order.iter().map(|&i| m.key[i]).collect();
        }
        Group { members }
    }

    pub fn member(&self, fluent: FluentId) -> Option<&Member> {
        self.members.iter().find(|m| m.fluent == fluent)
    }

    /// True if the values of some atoms are taken from one of their arguments (rather than from helper objects).
    pub fn has_values(&self) -> bool {
        self.members.iter().any(|m| m.value.is_some())
    }

    fn has_functional_key(&self) -> bool {
        (self.members.iter()).any(|m| m.key.iter().any(|k| matches!(k, KeyArg::Fun(..))))
    }
}

/// Specifies how the effects of an action on the atoms of a single key are rewritten.
/// Effects are designated by their index in the action's effects.
#[derive(Debug)]
pub struct KeyUpdate {
    /// Effect (an add) to be replaced by `sv(key) := value`
    pub assign: Option<usize>,
    /// Effect (a delete) to be replaced by `sv(key) := ⊥`
    pub erase: Option<usize>,
    /// Other effects on the atoms of this key, to be removed.
    pub removed: Vec<usize>,
    /// True if the update is precise because it deletes all other atoms of the key (P3'), rather than a required one.
    covering: bool,
}

impl KeyUpdate {
    /// True if the action holds the key between the erase and the assignment.
    fn holds_key(&self) -> bool {
        self.assign.is_some() && self.erase.is_some()
    }
}

/// The rewriting of an action's effects, as a list of updates on distinct keys.
pub type ActionUpdates = Vec<KeyUpdate>;

enum Failure {
    /// The group cannot be lifted, nor any of its supersets.
    Invalid(&'static str),
    /// An add effect is not balanced by a required delete, the given members would provide one.
    Unbalanced(Vec<Member>),
}

/// An add or delete effect on an atom of the group.
struct GroupEffect<'g> {
    idx: usize,
    member: &'g Member,
    is_add: bool,
    args: Vec<Term>,
    key: Vec<Term>,
    time: Timestamp,
}

struct ActionInfo<'a> {
    action: &'a Action,
    env: &'a Environment,
    guards: ActionGuards,
    /// Pairs of terms that the action requires to be distinct (`(not (= ?x ?y))`).
    distinct: Vec<(Term, Term)>,
}

impl<'a> ActionInfo<'a> {
    fn new(action: &'a Action, env: &'a Environment, bounds: &NumericBounds) -> Self {
        let mut distinct = Vec::new();
        for c in &action.conditions {
            for e in conjuncts(c.cond, env) {
                if let Expr::App(Fun::Not, args) = env.node(e).expr()
                    && args.len() == 1
                    && let Expr::App(Fun::Eq, args) = env.node(args[0]).expr()
                    && let Some([a, b]) = Term::all_of(args, env).as_deref()
                {
                    distinct.push((a.clone(), b.clone()));
                }
            }
        }
        ActionInfo {
            action,
            env,
            guards: ActionGuards::new(action, env, bounds),
            distinct,
        }
    }

    fn timing(&self) -> &ActionTiming {
        &self.guards.timing
    }

    /// The positive atoms required by the action.
    fn atoms(&self) -> impl Iterator<Item = &RequiredStateVariable> {
        (self.guards.requirements.iter()).filter(|r| r.value == RequiredValue::Bool(true))
    }

    /// False if the two terms necessarily denote distinct objects (according to types and inequality conditions).
    fn may_unify(&self, a: &Term, b: &Term) -> bool {
        let tpe = |t: &Term| match t {
            Term::Param(p) => self.action.parameters.iter().find(|q| q.name() == p).map(|q| q.tpe()),
            Term::Object(_) => None,
        };
        // types are disjoint if no object belongs to both (subtyping is not enough with multiple inheritance)
        let disjoint_types = match (tpe(a), tpe(b)) {
            (Some(ta), Some(tb)) => !self.env.objects.iter().any(|o| {
                let to = Type::from(o.tpe());
                to.is_subtype_of(ta) && to.is_subtype_of(tb)
            }),
            _ => false,
        };
        let declared_distinct = self
            .distinct
            .iter()
            .any(|(x, y)| (x == a && y == b) || (x == b && y == a));
        a.may_unify(b) && !disjoint_types && !declared_distinct
    }

    /// False if the two lists of terms necessarily denote distinct tuples of objects.
    fn may_unify_all(&self, a: &[Term], b: &[Term]) -> bool {
        a.iter().zip(b).all(|(x, y)| self.may_unify(x, y))
    }

    /// Value of a static function for the term, as given by a condition of the action.
    fn resolve(&self, f: &StaticFunction, input: &Term) -> Option<Term> {
        self.atoms()
            .filter(|r| r.fluent == f.fluent)
            .find_map(|r| f.apply(&r.args, input))
    }

    /// Returns the requirement that ensures that `fluent(args)` is true just before `t`.
    fn requirement(&self, fluent: FluentId, args: &[Term], t: Timestamp) -> Option<&RequiredStateVariable> {
        (self.guards.guards(fluent, args, t)).find(|r| r.value == RequiredValue::Bool(true))
    }
}

/// Analyses the model to find groups of predicates that can be lifted, and how to rewrite their effects.
pub struct Analyzer<'a> {
    model: &'a Model,
    non_positive: HashSet<FluentId>,
    /// Arguments of the atoms initially true for each predicate, or `None` if its initial state is not made of plain facts.
    init: HashMap<FluentId, Option<Vec<Vec<Term>>>>,
    actions: Vec<ActionInfo<'a>>,
    /// For each fluent, the indices (in `actions`) of the actions having an effect on it.
    modifiers: HashMap<FluentId, Vec<usize>>,
    domains: ParamDomains<'a>,
    functions: &'a StaticFunctions,
    occurrences: HashMap<FluentId, Vec<Occurrence>>,
}

/// Occurrence of a fluent in an expression: the action it appears in (if any) and its arguments (if simple terms).
type Occurrence = (Option<usize>, Option<Vec<Term>>);

impl<'a> Analyzer<'a> {
    pub fn new(model: &'a Model, functions: &'a StaticFunctions) -> Self {
        let env = &model.env;
        let mut init: HashMap<FluentId, Option<Vec<Vec<Term>>>> = HashMap::new();
        for eff in &model.init {
            let atoms = init
                .entry(eff.effect_expression.state_variable.fluent)
                .or_insert(Some(Vec::new()));
            match (init_atom(eff, env), atoms.as_mut()) {
                (Some(args), Some(atoms)) => atoms.push(args),
                _ => *atoms = None,
            }
        }

        let bounds = NumericBounds::new(model);
        let actions: Vec<ActionInfo> = (model.actions.iter())
            .map(|a| ActionInfo::new(a, env, &bounds))
            .collect();
        let mut modifiers: HashMap<FluentId, Vec<usize>> = HashMap::new();
        for (i, a) in actions.iter().enumerate() {
            for eff in &a.action.effects {
                let users = modifiers
                    .entry(eff.effect_expression.state_variable.fluent)
                    .or_default();
                if users.last() != Some(&i) {
                    users.push(i);
                }
            }
        }
        let mut occurrences: HashMap<FluentId, Vec<_>> = HashMap::new();
        let roots = (model_roots(model).into_iter().map(|r| (None, r))).chain(
            (model.actions.iter().enumerate())
                .flat_map(|(i, a)| action_roots(a).into_iter().map(move |r| (Some(i), r))),
        );
        for (action, (root, _)) in roots {
            for sv in state_variables(root, env) {
                if let Expr::StateVariable(fluent, args) = env.node(sv).expr() {
                    let occurrence = (action, Term::all_of(args, env));
                    occurrences.entry(*fluent).or_default().push(occurrence);
                }
            }
        }
        Analyzer {
            model,
            non_positive: fluents_with_non_positive_occurrences(model),
            init,
            actions,
            modifiers,
            domains: ParamDomains::new(model),
            functions,
            occurrences,
        }
    }

    /// Returns all valid groups (of at most `MAX_GROUP_SIZE` predicates).
    ///
    /// Starting from single predicates, a candidate group that has an unbalanced add effect in an action
    /// is extended with a predicate that the action requires and deletes.
    pub fn valid_groups(&self) -> Vec<Group> {
        let mut queue: VecDeque<Group> = VecDeque::new();
        for (fluent, f) in self.model.env.fluents.iter_with_id() {
            if f.return_type != Type::Bool {
                continue;
            }
            let arity = f.parameters.len();
            for value in std::iter::once(None).chain((0..arity).map(Some)) {
                let key = (0..arity).filter(|&i| Some(i) != value).map(KeyArg::Arg).collect();
                queue.push_back(Group::new(vec![Member { fluent, key, value }]));
            }
        }
        let mut seen: HashSet<Group> = queue.iter().cloned().collect();
        let mut valid = Vec::new();

        while let Some(group) = queue.pop_front() {
            let res = self.updates(&group);
            match res {
                Ok(_) => valid.push(group),
                Err(Failure::Unbalanced(extensions)) => {
                    for ext in extensions {
                        let mut members = group.members.clone();
                        members.push(ext);
                        let extended = Group::new(members);
                        if extended.members.len() <= MAX_GROUP_SIZE && seen.insert(extended.clone()) {
                            queue.push_back(extended);
                        }
                    }
                }
                Err(Failure::Invalid(reason)) => tracing::trace!("{group:?}: {reason}"),
            }
        }
        valid
    }

    /// Key of an atom of the member, in the context of an action (if any) to evaluate static functions.
    fn key(&self, member: &Member, args: &[Term], action: Option<&ActionInfo>) -> Option<Vec<Term>> {
        member.key_of(args, |f, input| match input {
            Term::Object(o) => self.functions.eval(f, o).cloned().map(Term::Object),
            Term::Param(_) => action?.resolve(f, input),
        })
    }

    /// Number of ground keys of the group, i.e. of state variables of the lifted state function.
    pub fn num_keys(&self, group: &Group) -> usize {
        let env = &self.model.env;
        let m = &group.members[0];
        (m.key.iter())
            .map(|k| {
                let tpe = match *k {
                    KeyArg::Arg(i) => env.fluents.get(m.fluent).parameters[i].tpe(),
                    KeyArg::Fun(f, _) => env.fluents.get(f.fluent).parameters[f.output()].tpe(),
                };
                env.objects
                    .iter()
                    .filter(|o| Type::from(o.tpe()).is_subtype_of(tpe))
                    .count()
            })
            .fold(1, usize::saturating_mul)
    }

    /// Actions having an effect on the fluent.
    fn modifiers(&self, fluent: FluentId) -> impl Iterator<Item = &ActionInfo<'a>> {
        let indices = self.modifiers.get(&fluent).map_or(&[][..], |v| v.as_slice());
        indices.iter().map(|&i| &self.actions[i])
    }

    /// Add effects on the fluent, with the action they belong to.
    fn adds(&self, fluent: FluentId) -> impl Iterator<Item = (&'a Action, &'a SimpleEffect)> {
        let env = &self.model.env;
        self.modifiers(fluent).flat_map(move |a| {
            a.action
                .effects
                .iter()
                .map(|eff| &eff.effect_expression)
                .filter(move |e| e.state_variable.fluent == fluent && is_add(&e.operation, env))
                .map(move |e| (a.action, e))
        })
    }

    /// True if an atom of the group may be true at some point (initially or after an action).
    pub fn may_be_true(&self, group: &Group) -> bool {
        group.members.iter().any(|m| {
            self.init
                .get(&m.fluent)
                .is_some_and(|atoms| atoms.as_ref().is_none_or(|a| !a.is_empty()))
                || self.adds(m.fluent).next().is_some()
        })
    }

    /// Returns, for each action (with its name), how to rewrite its effects on the group.
    /// Fails if the group cannot be lifted.
    pub fn rewriting(&self, group: &Group) -> Option<Vec<(Sym, ActionUpdates)>> {
        self.updates(group).ok()
    }

    fn updates(&self, group: &Group) -> Result<Vec<(Sym, ActionUpdates)>, Failure> {
        self.check_global(group).map_err(Failure::Invalid)?;
        // only the actions with an effect on the group need to be considered
        let mut actions: Vec<usize> = (group.members.iter())
            .flat_map(|m| self.modifiers.get(&m.fluent).into_iter().flatten().copied())
            .collect();
        actions.sort();
        actions.dedup();
        let updates: Vec<(Sym, ActionUpdates)> = actions
            .into_iter()
            .map(|i| &self.actions[i])
            .map(|a| Ok((a.action.name.clone(), self.action_updates(group, a)?)))
            .collect::<Result<_, Failure>>()?;
        // (P3') updates without requirement are only valid if no action may hold a key
        let all_updates = || updates.iter().flat_map(|(_, u)| u);
        if all_updates().any(|u| u.covering) && all_updates().any(|u| u.holds_key()) {
            return Err(Failure::Invalid(
                "key held by an action while another one relies on covering deletes",
            ));
        }
        Ok(updates)
    }

    /// Checks the conditions that do not depend on actions. If they fail for a group, they fail for any superset.
    fn check_global(&self, group: &Group) -> Result<(), &'static str> {
        let fluents = &self.model.env.fluents;
        let param_type = |m: &Member, i: usize| fluents.get(m.fluent).parameters[i].tpe().clone();
        let key_type = |m: &Member, k: usize| match m.key[k] {
            KeyArg::Arg(i) => param_type(m, i),
            KeyArg::Fun(f, _) => fluents.get(f.fluent).parameters[f.output()].tpe().clone(),
        };

        for m in &group.members {
            if fluents.get(m.fluent).return_type != Type::Bool {
                return Err("not a predicate");
            }
            if self.non_positive.contains(&m.fluent) {
                return Err("occurs in a non-positive context");
            }
            if self.init.get(&m.fluent).is_some_and(|atoms| atoms.is_none()) {
                return Err("non-plain initial fact");
            }
        }
        for i in 0..group.members[0].key.len() {
            supertype(group.members.iter().map(|m| key_type(m, i))).ok_or("incompatible key types")?;
        }
        let valued = || group.members.iter().filter(|m| m.value.is_some());
        if valued().any(|m| !matches!(param_type(m, m.value.unwrap()), Type::User(_))) {
            return Err("value is not an object");
        }

        // functional keys must be computable for every occurrence of the atoms
        if group.has_functional_key() {
            for m in &group.members {
                for (action, args) in self.occurrences.get(&m.fluent).into_iter().flatten() {
                    let action = action.map(|i| &self.actions[i]);
                    if args.as_ref().and_then(|args| self.key(m, args, action)).is_none() {
                        return Err("functional key not computable for an occurrence");
                    }
                }
            }
        }

        // at most one atom per key in the initial state
        let mut init_atoms: HashMap<Vec<Term>, (FluentId, Option<Term>)> = HashMap::new();
        for m in &group.members {
            for args in self.init.get(&m.fluent).into_iter().flatten().flatten() {
                let atom = (m.fluent, m.value_of(args));
                let key = self
                    .key(m, args, None)
                    .ok_or("initial atom with undefined functional key")?;
                if *init_atoms.entry(key).or_insert(atom.clone()) != atom {
                    return Err("several atoms of the same key in the initial state");
                }
            }
        }

        // the values of distinct predicates must be distinct
        if group.has_values() && group.members.len() > 1 {
            let values: Vec<HashSet<Sym>> = valued().map(|m| self.possible_values(m)).collect();
            for (i, vi) in values.iter().enumerate() {
                if values[i + 1..].iter().any(|vj| !vi.is_disjoint(vj)) {
                    return Err("predicates may have the same value");
                }
            }
        }
        Ok(())
    }

    /// Over-approximation of the objects that may appear as the value of an atom of this member.
    fn possible_values(&self, m: &Member) -> HashSet<Sym> {
        let env = &self.model.env;
        let value_pos = m.value.unwrap();
        let value_type = env.fluents.get(m.fluent).parameters[value_pos].tpe();
        let mut values = HashSet::new();
        for args in self.init.get(&m.fluent).into_iter().flatten().flatten() {
            if let Term::Object(o) = &args[value_pos] {
                values.insert(o.clone());
            }
        }
        for (action, e) in self.adds(m.fluent) {
            match Term::of(e.state_variable.arguments[value_pos], env) {
                Some(Term::Object(o)) => {
                    values.insert(o);
                }
                Some(Term::Param(p)) => values.extend(self.domains.domain(action, &p, value_type)),
                None => values.extend(self.domains.domain(action, &Sym::from(""), value_type)),
            }
        }
        values
    }

    /// Computes how the effects of the action on the group are rewritten (pattern P of the module documentation).
    fn action_updates(&self, group: &Group, info: &ActionInfo) -> Result<ActionUpdates, Failure> {
        let env = &self.model.env;
        let timing = info.timing();

        // effects on the group, partitioned by key
        let mut partitions: Vec<Vec<GroupEffect>> = Vec::new();
        for (idx, eff) in info.action.effects.iter().enumerate() {
            let e = &eff.effect_expression;
            let Some(member) = group.member(e.state_variable.fluent) else {
                continue;
            };
            if !eff.universal_quantification.is_empty() || e.condition.is_some() {
                return Err(Failure::Invalid("quantified or conditional effect"));
            }
            let is_add = is_add(&e.operation, env);
            if !is_add && !is_delete(&e.operation, env) {
                return Err(Failure::Invalid("non-constant effect"));
            }
            let args = Term::all_of(&e.state_variable.arguments, env).ok_or(Failure::Invalid("non-simple args"))?;
            let key = (self.key(member, &args, Some(info))).ok_or(Failure::Invalid("functional key not computable"))?;
            let effect = GroupEffect {
                idx,
                member,
                is_add,
                args,
                key,
                time: e.transition_start,
            };
            match partitions.iter_mut().find(|p| p[0].key == effect.key) {
                Some(partition) => partition.push(effect),
                None => partitions.push(vec![effect]),
            }
        }

        let mut updates = Vec::with_capacity(partitions.len());
        let mut required = Vec::with_capacity(partitions.len());
        for partition in &partitions {
            let (update, req) = self.key_update(group, info, partition)?;
            updates.push(update);
            required.push(req);
        }

        // (P4) distinct keys that may denote the same object must be associated to incompatible requirements
        for i in 0..partitions.len() {
            for j in (i + 1)..partitions.len() {
                let (ki, kj) = (&partitions[i][0].key, &partitions[j][0].key);
                if !info.may_unify_all(ki, kj) {
                    continue;
                }
                let (Some(ri), Some(rj)) = (required[i], required[j]) else {
                    return Err(Failure::Invalid("updates on keys that may unify"));
                };
                let (mi, mj) = (group.member(ri.fluent).unwrap(), group.member(rj.fluent).unwrap());
                let distinct_atoms = match (mi.value_of(&ri.args), mj.value_of(&rj.args)) {
                    _ if ri.fluent != rj.fluent => true,
                    (Some(vi), Some(vj)) => !info.may_unify(&vi, &vj),
                    _ => false,
                };
                if !distinct_atoms || !timing.hold_together(ri.interval, rj.interval) {
                    return Err(Failure::Invalid("updates on keys that may unify"));
                }
            }
        }
        Ok(updates)
    }

    /// Checks that the effects on a single key follow pattern P, and returns the corresponding update
    /// together with the requirement that the update relies on.
    fn key_update<'i>(
        &self,
        group: &Group,
        info: &'i ActionInfo,
        effects: &[GroupEffect],
    ) -> Result<(KeyUpdate, Option<&'i RequiredStateVariable>), Failure> {
        let timing = info.timing();
        let (adds, dels): (Vec<&GroupEffect>, Vec<&GroupEffect>) = effects.iter().partition(|e| e.is_add);

        // (P2) a single atom added, at a single time
        let add = adds.first();
        for a in adds.iter().skip(1) {
            if a.member.fluent != adds[0].member.fluent || a.args != adds[0].args {
                return Err(Failure::Invalid("several atoms added for the same key"));
            }
            if !timing.eq(a.time, adds[0].time) {
                return Err(Failure::Invalid("atom added at distinct times"));
            }
        }

        // (P3) the first delete must be required and others may not happen after the add.
        let first_del = dels.iter().find(|d| dels.iter().all(|o| timing.le(d.time, o.time)));
        if first_del.is_none() && !dels.is_empty() {
            return Err(Failure::Invalid("deletes are not ordered"));
        }
        // time of the first delete or, if none, of the add
        let t_ref = first_del.or(add).unwrap().time;
        if let Some(add) = add {
            if !dels.iter().all(|d| timing.le(d.time, add.time)) {
                return Err(Failure::Invalid("delete after add"));
            }
        } else if !dels.iter().all(|d| timing.eq(d.time, t_ref)) {
            return Err(Failure::Invalid("deletes at distinct times without add"));
        }
        let requirement = dels
            .iter()
            .filter(|d| timing.eq(d.time, t_ref))
            .find_map(|d| info.requirement(d.member.fluent, &d.args, t_ref));
        // (P3') without requirement, all atoms of the key (but the added one) must be deleted at the time of the add
        // (not used for single predicates, as it would prevent their extension into a useful group)
        let covering = requirement.is_none()
            && !group.has_values()
            && group.members.len() > 1
            && add.is_none_or(|a| timing.eq(a.time, t_ref))
            && group.members.iter().all(|m| {
                add.is_some_and(|a| a.member == m) || dels.iter().any(|d| d.member == m && timing.eq(d.time, t_ref))
            });
        if requirement.is_none() && !covering {
            return Err(Failure::Unbalanced(self.extensions(
                group,
                info,
                &effects[0].key,
                t_ref,
            )));
        }

        let assign = add.map(|a| a.idx);
        let erase = match (first_del, add) {
            (Some(d), Some(a)) if timing.eq(a.time, d.time) => None,
            (d, _) => d.map(|d| d.idx),
        };
        let removed = effects
            .iter()
            .map(|e| e.idx)
            .filter(|&i| Some(i) != assign && Some(i) != erase)
            .collect();
        let update = KeyUpdate {
            assign,
            erase,
            removed,
            covering,
        };
        Ok((update, requirement))
    }

    /// Returns the predicates that could be added to the group so that the action would delete a required atom
    /// of the given key, no later than `t` (P3).
    /// For groups without values, also returns the predicates without value deleted at `t`, which may allow (P3').
    fn extensions(&self, group: &Group, info: &ActionInfo, key: &[Term], t: Timestamp) -> Vec<Member> {
        let env = &self.model.env;
        let mut extensions = Vec::new();
        for eff in &info.action.effects {
            let e = &eff.effect_expression;
            let fluent = e.state_variable.fluent;
            if group.member(fluent).is_some()
                || !eff.universal_quantification.is_empty()
                || e.condition.is_some()
                || !is_delete(&e.operation, env)
                || !info.timing().le(e.transition_start, t)
            {
                continue;
            }
            let Some(args) = Term::all_of(&e.state_variable.arguments, env) else {
                continue;
            };
            // each key term must be an argument of the atom (or the image of one by a static function),
            // with at most one remaining argument: the value (which is the input of all static functions)
            let components: Option<Vec<KeyArg>> = key.iter().map(|k| self.key_arg(info, &args, k)).collect();
            let Some(components) = components else { continue };
            let mut others = (0..args.len()).filter(|i| !components.contains(&KeyArg::Arg(*i)));
            let value = others.next();
            let inputs_are_value = components.iter().all(|c| match c {
                KeyArg::Arg(_) => true,
                KeyArg::Fun(_, i) => Some(*i) == value,
            });
            let required = info.requirement(fluent, &args, e.transition_start).is_some();
            let covering = !group.has_values() && value.is_none() && info.timing().eq(e.transition_start, t);
            if others.next().is_none() && inputs_are_value && (required || covering) {
                extensions.push(Member {
                    fluent,
                    key: components,
                    value,
                });
            }
        }
        extensions
    }

    /// How the term `k` can be obtained from the arguments of an atom in the action: as an argument appearing exactly once,
    /// or (if it is not an argument) as the image of an argument by a static function, given by a condition of the action.
    fn key_arg(&self, info: &ActionInfo, args: &[Term], k: &Term) -> Option<KeyArg> {
        match args.iter().positions(|a| a == k).at_most_one().ok()? {
            Some(i) => Some(KeyArg::Arg(i)),
            None => info.atoms().filter(|r| r.args.len() == 2).find_map(|r| {
                let f = [0, 1]
                    .map(|input| StaticFunction {
                        fluent: r.fluent,
                        input,
                    })
                    .into_iter()
                    .find(|f| &r.args[f.output()] == k && self.functions.is_function(f))?;
                let i = args.iter().positions(|a| *a == r.args[f.input]).exactly_one().ok()?;
                Some(KeyArg::Fun(f, i))
            }),
        }
    }
}

fn is_add(op: &EffectOp, env: &Environment) -> bool {
    matches!(op, EffectOp::Assign(v) if matches!(env.node(*v).expr(), Expr::Bool(true)))
}

fn is_delete(op: &EffectOp, env: &Environment) -> bool {
    match op {
        EffectOp::Assign(v) => matches!(env.node(*v).expr(), Expr::Bool(false)),
        EffectOp::Erase => true,
        EffectOp::Increase(_) | EffectOp::Decrease(_) => false,
    }
}

/// Returns a type among the given ones that is a supertype of all the others (if any).
pub fn supertype(types: impl IntoIterator<Item = Type>) -> Option<Type> {
    let types: Vec<Type> = types.into_iter().collect();
    types
        .iter()
        .find(|t| types.iter().all(|other| other.is_subtype_of(t)))
        .cloned()
}
