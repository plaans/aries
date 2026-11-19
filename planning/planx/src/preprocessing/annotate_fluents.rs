//! Annotation of fluents with properties derived from their usage in the model (see [`FluentProperties`]).

use std::collections::{HashMap, HashSet};

use crate::preprocessing::analysis::{
    StaticFunction, StaticFunctions, Term, fluents_modified_by_actions, fluents_read, init_atom, numeric_properties,
    opaque_fluents,
};
use crate::*;

/// Annotates all fluents of the model with their [`FluentProperties`] (replacing any previous annotation).
pub fn annotate_fluents(model: &mut Model) -> Res<()> {
    let modified = fluents_modified_by_actions(model);
    let read = fluents_read(model);
    let opaque = opaque_fluents(model);
    let functions = StaticFunctions::new(model);
    let numeric = numeric_properties(model);
    let mut props = HashMap::new();
    for (id, fluent) in model.env.fluents.iter_with_id() {
        let is_static = !modified.contains(&id);
        let binary_relation = is_static && fluent.return_type == Type::Bool && fluent.parameters.len() == 2;
        let functional_inputs = if binary_relation {
            (0..2)
                .filter(|&input| functions.is_function(&StaticFunction { fluent: id, input }))
                .collect()
        } else {
            Vec::new()
        };
        let acyclic = if binary_relation && fluent.parameters[0].tpe() == fluent.parameters[1].tpe() {
            longest_path(id, model)
        } else {
            None
        };
        props.insert(
            id,
            FluentProperties {
                is_static,
                read: read.contains(&id),
                opaque: opaque.contains(&id),
                functional_inputs,
                acyclic,
                numeric: numeric.get(&id).cloned(),
            },
        );
    }
    for (id, p) in props {
        model.env.fluents.get_mut(id).annotations.properties = Some(p);
    }
    Ok(())
}

/// If the static binary relation, seen as a graph over objects, is acyclic, returns the number of edges in its longest
/// path. Returns `None` if it has a cycle or if its initial value cannot be interpreted.
fn longest_path(fluent: FluentId, model: &Model) -> Option<usize> {
    let mut succs: HashMap<Sym, Vec<Sym>> = HashMap::new();
    for eff in &model.init {
        if eff.effect_expression.state_variable.fluent != fluent {
            continue;
        }
        match init_atom(eff, &model.env)?.as_slice() {
            [Term::Object(a), Term::Object(b)] => succs.entry(a.clone()).or_default().push(b.clone()),
            _ => return None,
        }
    }
    // depth-first search computing the longest path from each node, detecting cycles
    fn visit(
        n: &Sym,
        succs: &HashMap<Sym, Vec<Sym>>,
        done: &mut HashMap<Sym, usize>,
        active: &mut HashSet<Sym>,
    ) -> Option<usize> {
        if let Some(&l) = done.get(n) {
            return Some(l);
        }
        if !active.insert(n.clone()) {
            return None; // cycle
        }
        let mut l = 0;
        for s in succs.get(n).into_iter().flatten() {
            l = l.max(1 + visit(s, succs, done, active)?);
        }
        active.remove(n);
        done.insert(n.clone(), l);
        Some(l)
    }
    let mut done = HashMap::new();
    let mut active = HashSet::new();
    let mut longest = 0;
    for n in succs.keys() {
        longest = longest.max(visit(n, &succs, &mut done, &mut active)?);
    }
    Some(longest)
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::preprocessing::annotate_fluents;
    use crate::*;

    fn build(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        build_model(&domain, &problem).unwrap()
    }

    fn props(model: &Model, name: &str) -> FluentProperties {
        let id = model.env.fluents.get_by_name(name).unwrap();
        model.env.fluents.get(id).annotations.properties.clone().unwrap()
    }

    #[test]
    fn fluent_properties() {
        let domain = "(define (domain d) (:requirements :typing :negative-preconditions :numeric-fluents)
            (:types block place)
            (:predicates (above ?x - block ?y - block) (loop ?x - place ?y - place) (at ?b - block ?p - place)
                         (link ?b - block ?p - place) (clear ?b - block) (seen ?b - block) (done ?b - block))
            (:functions (fuel) (cost))
            (:action move :parameters (?b - block ?from - place ?to - place)
              :precondition (and (at ?b ?from) (loop ?from ?to) (link ?b ?to) (>= (fuel) 1) (not (seen ?b)))
              :effect (and (not (at ?b ?from)) (at ?b ?to) (decrease (fuel) 1) (increase (cost) 1) (seen ?b)))
            (:action finish :parameters (?b - block ?c - block)
              :precondition (and (above ?b ?c) (clear ?b))
              :effect (and (done ?b))))";
        let problem = "(define (problem p) (:domain d)
            (:objects a b c - block p1 p2 - place)
            (:init (above a b) (above b c) (loop p1 p2) (loop p2 p1) (link a p1) (link b p1) (at a p1)
                   (clear a) (= (fuel) 3) (= (cost) 0))
            (:goal (done a)) (:metric minimize (cost)))";
        let mut model = build(domain, problem);
        annotate_fluents(&mut model).unwrap();
        let above = props(&model, "above");
        assert!(above.is_static && above.read && !above.opaque);
        assert_eq!(above.acyclic, Some(2));
        assert_eq!(above.functional_inputs, vec![0, 1]);
        let lp = props(&model, "loop");
        assert_eq!(lp.acyclic, None);
        assert_eq!(lp.functional_inputs, vec![0, 1]);
        // `link` maps both a and b to p1: functional from blocks only
        assert_eq!(props(&model, "link").functional_inputs, vec![0]);
        let at = props(&model, "at");
        assert!(!at.is_static && !at.opaque);
        // negative precondition: still a simple requirement
        assert!(!props(&model, "seen").opaque);
        let fuel = props(&model, "fuel");
        assert!(fuel.opaque);
        let fuel = fuel.numeric.unwrap();
        assert_eq!(
            (fuel.increase, fuel.decrease),
            (DirectionImpact::Helpful, DirectionImpact::Harmful)
        );
        assert!(fuel.never_increased && !fuel.never_decreased);
        let cost = props(&model, "cost");
        assert!(cost.opaque);
        let cost = cost.numeric.unwrap();
        assert_eq!(
            (cost.increase, cost.decrease),
            (DirectionImpact::Harmful, DirectionImpact::Helpful)
        );
        assert!(!cost.never_increased && cost.never_decreased);
        assert!(!props(&model, "done").opaque);
    }
}
