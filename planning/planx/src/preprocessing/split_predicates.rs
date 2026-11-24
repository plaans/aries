//! Splitting of predicates according to the types of their arguments.
//!
//! A predicate whose occurrences each only apply to a part of the objects of an argument's type is split into
//! one predicate per part. For instance, in depots, `at(?x - locatable, ?p - place)` is used with trucks, hoists and
//! surfaces (but never with a parameter that could be, e.g., a truck or a hoist), and is split into
//! `at-truck`, `at-hoist` and `at-surface`. Each of them may then be lifted independently into a state function.
//!
//! # Why this is correct
//!
//! For each argument position, the object types are partitioned so that, for every occurrence of the predicate
//! (in a condition, effect, initial fact, goal, ...), the objects that the argument may denote (by its static type)
//! all belong to a single part. An atom `p(o₁, ..., oₙ)` is mapped to the atom of the predicate associated with the
//! parts of `o₁, ..., oₙ`, which is a bijection. Every occurrence of `p` is replaced by the predicate of its parts,
//! and can only denote atoms that are mapped to this predicate: each state of the original model thus corresponds to
//! exactly one state of the transformed model, with the same value for all conditions.

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap};

use itertools::Itertools;

use crate::errors::{EnvError, Spanned};
use crate::preprocessing::analysis::{expression_roots, state_variables};
use crate::*;

/// Splits predicates into several ones when their occurrences only apply to disjoint subsets of objects
/// (see the module documentation).
pub fn split_predicates(model: &mut Model) -> Res<()> {
    let env = &model.env;
    let occurrences = occurrences(model);

    // for each predicate to split, the part (one class per argument position) of each occurrence
    let mut splits: Vec<(FluentId, Vec<PartOccurrence>, Vec<Partition>)> = Vec::new();
    for (fluent, f) in env.fluents.iter_with_id() {
        if f.return_type != Type::Bool {
            continue;
        }
        let occs = occurrences.get(&fluent).map_or(&[][..], |o| o.as_slice());
        let partitions: Option<Vec<Partition>> = (0..f.parameters.len())
            .map(|i| Partition::new(f.parameters[i].tpe(), occs.iter().map(|o| o.args(model)[i]), env))
            .collect();
        let Some(partitions) = partitions else { continue };
        let parts: Vec<PartOccurrence> = (occs.iter())
            .map(|o| {
                let args = o.args(model);
                let part = (0..args.len()).map(|i| partitions[i].class_of(args[i], env)).collect();
                (o.clone(), part)
            })
            .collect();
        if parts.iter().map(|(_, part)| part).all_equal() {
            continue;
        }
        splits.push((fluent, parts, partitions));
    }
    if splits.is_empty() {
        return Ok(());
    }

    println!("Splitting predicates:");
    for (fluent, parts, partitions) in splits {
        let f = model.env.fluents.get(fluent).clone();
        let mut new_fluents: HashMap<Vec<usize>, FluentId> = HashMap::new();
        // positions on which the predicate is split
        let split_positions = (0..partitions.len())
            .filter(|&i| !parts.iter().map(|(_, part)| part[i]).all_equal())
            .collect_vec();
        for part in parts.iter().map(|(_, part)| part).unique().sorted() {
            let mut name = f.name().to_string();
            let mut params = f.parameters.clone();
            for (i, partition) in partitions.iter().enumerate() {
                if split_positions.contains(&i) {
                    let tpe = partition.class_types[part[i]].clone();
                    name = format!("{name}-{tpe}");
                    params[i] = Param::new(params[i].name().clone(), tpe);
                }
            }
            while model.env.fluents.get_by_name(&name).is_some() {
                name.push('_');
            }
            let id = (model.env.fluents)
                .add_fluent(name.as_str(), params, Type::Bool, f.source.clone())
                .msg(&model.env)?;
            new_fluents.insert(part.clone(), id);
        }
        println!(
            "  - {:<40} [into: {}]",
            f.name().to_string(),
            new_fluents
                .values()
                .map(|&id| model.env.fluents.get(id).name())
                .sorted()
                .join(", ")
        );
        for (occurrence, part) in parts {
            occurrence.replace_fluent(new_fluents[&part], model)?;
        }
        model.env.fluents.remove(|id, _| id == fluent);
    }
    Ok(())
}

/// An occurrence with its part: the class of each of its arguments.
type PartOccurrence = (Occurrence, Vec<usize>);

/// A place where a predicate occurs in the model.
#[derive(Clone, Debug)]
enum Occurrence {
    /// A state variable expression.
    Expr(ExprId),
    /// The `i`-th effect of an action.
    ActionEffect(Sym, usize),
    /// The `i`-th effect of the initial state.
    InitEffect(usize),
}

impl Occurrence {
    fn args(&self, model: &Model) -> SeqExprId {
        match self {
            Occurrence::Expr(e) => match model.env.node(*e).expr() {
                Expr::StateVariable(_, args) => args.clone(),
                _ => unreachable!(),
            },
            Occurrence::ActionEffect(a, i) => {
                let action = model.actions.get_action(a).unwrap();
                action.effects[*i].effect_expression.state_variable.arguments.clone()
            }
            Occurrence::InitEffect(i) => model.init[*i].effect_expression.state_variable.arguments.clone(),
        }
    }

    fn replace_fluent(&self, fluent: FluentId, model: &mut Model) -> Res<()> {
        match self {
            Occurrence::Expr(e) => {
                let args = self.args(model);
                let span = model.env.node(*e).span().cloned();
                model.env.replace(*e, Expr::StateVariable(fluent, args), span)?;
            }
            Occurrence::ActionEffect(a, i) => {
                let action = model.actions.get_action_mut(a).unwrap();
                action.effects[*i].effect_expression.state_variable.fluent = fluent;
            }
            Occurrence::InitEffect(i) => model.init[*i].effect_expression.state_variable.fluent = fluent,
        }
        Ok(())
    }
}

/// All occurrences of each fluent in the model.
fn occurrences(model: &Model) -> HashMap<FluentId, Vec<Occurrence>> {
    let env = &model.env;
    let mut occurrences: HashMap<FluentId, Vec<Occurrence>> = HashMap::new();
    for (root, _) in expression_roots(model) {
        for e in state_variables(root, env) {
            if let Expr::StateVariable(fluent, _) = env.node(e).expr() {
                occurrences.entry(*fluent).or_default().push(Occurrence::Expr(e));
            }
        }
    }
    for action in model.actions.iter() {
        for (i, eff) in action.effects.iter().enumerate() {
            let fluent = eff.effect_expression.state_variable.fluent;
            occurrences
                .entry(fluent)
                .or_default()
                .push(Occurrence::ActionEffect(action.name.clone(), i));
        }
    }
    for (i, eff) in model.init.iter().enumerate() {
        let fluent = eff.effect_expression.state_variable.fluent;
        occurrences.entry(fluent).or_default().push(Occurrence::InitEffect(i));
    }
    occurrences
}

/// A partition of the object types of an argument such that each occurrence only applies to objects of a single class.
struct Partition {
    /// Class of the objects of each type of the occurrences (by name).
    classes: HashMap<String, usize>,
    /// For each class, the most specific type (among the ones of the occurrences) containing all its objects.
    class_types: Vec<Type>,
}

impl Partition {
    /// Builds the finest partition compatible with the given occurrences (as the arguments at this position).
    /// Returns `None` if some occurrence cannot be assigned to a class (non-object type, or no possible object).
    fn new(tpe: &Type, args: impl Iterator<Item = ExprId>, env: &Environment) -> Option<Self> {
        if !matches!(tpe, Type::User(_)) {
            return None;
        }
        let object_types = |t: &Type| -> BTreeSet<String> {
            (env.objects.iter())
                .filter(|o| Type::from(o.tpe()).is_subtype_of(t))
                .map(|o| o.tpe().to_string())
                .collect()
        };
        // object types that each type of the occurrences may denote
        let mut covered: HashMap<String, (Type, BTreeSet<String>)> = HashMap::new();
        for arg in args {
            let arg_tpe = env.node(arg).tpe().clone();
            if let Entry::Vacant(entry) = covered.entry(arg_tpe.to_string()) {
                let types = object_types(&arg_tpe);
                if types.is_empty() {
                    return None;
                }
                entry.insert((arg_tpe, types));
            }
        }

        // union-find over object types, each type of the occurrences merging the object types it covers
        let mut parent: HashMap<String, String> = object_types(tpe).into_iter().map(|t| (t.clone(), t)).collect();
        fn root(parent: &HashMap<String, String>, t: &str) -> String {
            let mut t = t.to_string();
            while parent[&t] != t {
                t = parent[&t].clone();
            }
            t
        }
        for (_, types) in covered.values() {
            let first = root(&parent, types.first().unwrap());
            for t in types {
                let r = root(&parent, t);
                parent.insert(r, first.clone());
            }
        }
        let roots: Vec<String> = parent.keys().map(|t| root(&parent, t)).unique().sorted().collect();
        let class = |t: &str| roots.iter().position(|r| *r == root(&parent, t)).unwrap();

        let classes: HashMap<String, usize> = (covered.iter())
            .map(|(name, (_, types))| (name.clone(), class(types.first().unwrap())))
            .collect();
        // the type of a class: a type of its occurrences that is a supertype of all the others (if any)
        let class_types = (0..roots.len())
            .map(|c| {
                let types = (covered.iter())
                    .filter(|(name, _)| classes[*name] == c)
                    .map(|(_, (t, _))| t)
                    .collect_vec();
                (types.iter())
                    .find(|t| types.iter().all(|other| other.is_subtype_of(t)))
                    .map_or(tpe, |t| *t)
                    .clone()
            })
            .collect();
        Some(Partition { classes, class_types })
    }

    /// Class of the objects that the expression may denote.
    fn class_of(&self, arg: ExprId, env: &Environment) -> usize {
        self.classes[&env.node(arg).tpe().to_string()]
    }
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::*;
    use itertools::Itertools;

    fn split(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let mut model = build_model(&domain, &problem).unwrap();
        super::split_predicates(&mut model).unwrap();
        println!("{model}");
        model
    }

    fn fluents(model: &Model) -> Vec<String> {
        model
            .env
            .fluents
            .iter()
            .map(|f| f.name().to_string())
            .sorted()
            .collect()
    }

    fn domain(extra: &str) -> String {
        format!(
            "(define (domain d) (:requirements :typing)
               (:types place locatable - object truck package - locatable)
               (:predicates (at ?x - locatable ?p - place) (in ?p - package ?t - truck))
               (:action drive :parameters (?t - truck ?from ?to - place)
                 :precondition (at ?t ?from) :effect (and (not (at ?t ?from)) (at ?t ?to)))
               {extra}
               (:action load :parameters (?x - package ?t - truck ?l - place)
                 :precondition (and (at ?x ?l) (at ?t ?l)) :effect (and (not (at ?x ?l)) (in ?x ?t))))"
        )
    }

    const PB: &str = "(define (problem p) (:domain d) (:objects t - truck x - package l1 l2 - place)
        (:init (at t l1) (at x l1)) (:goal (and (at t l2) (in x t))))";

    #[test]
    fn split_by_type() {
        let model = split(&domain(""), PB);
        assert_eq!(fluents(&model), ["at-package", "at-truck", "in"]);
        let load = model.actions.get_action(&Sym::from("load")).unwrap();
        assert_eq!(
            model
                .env
                .fluents
                .get(load.effects[0].effect_expression.state_variable.fluent)
                .name(),
            "at-package"
        );
    }

    #[test]
    fn no_split_with_general_occurrence() {
        // `(at ?x ?l)` in `inspect` may apply to both trucks and packages
        let inspect =
            "(:action inspect :parameters (?x - locatable ?l - place) :precondition (at ?x ?l) :effect (and))";
        let model = split(&domain(inspect), PB);
        assert_eq!(fluents(&model), ["at", "in"]);
    }
}
