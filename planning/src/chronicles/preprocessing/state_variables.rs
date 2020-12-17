use crate::chronicles::{Chronicle, Effect, Problem, StateFun};
use aries_model::lang::*;
use aries_model::symbols::{SymId, TypedSym};
use itertools::Itertools;
use std::collections::HashMap;
use std::convert::TryFrom;

/// Substitute predicates into state functions where applicable.
/// For instance the predicate `(at agent location) -> boolean` can usually be
/// transformed into the state function `(at agent) -> location`.
/// For this transformation to be applicable, it should be the case that,
/// for a given `agent`, there is at most one `location` such that `(at agent location) = true`.
pub fn predicates_as_state_variables(pb: &mut Problem) {
    let to_substitute: Vec<SymId> = pb
        .context
        .state_functions
        .iter()
        .filter(|sf| substitutable(pb, sf))
        .map(|sf| sf.sym)
        .collect();
    if !to_substitute.is_empty() {
        println!("Substitution from predicate to state variable:")
    }
    for &sf in &to_substitute {
        println!(" - {}", pb.context.model.symbols.symbol(sf));
    }
    to_state_variables(pb, &to_substitute)
}

fn to_state_variables(pb: &mut Problem, state_functions: &[SymId]) {
    let sub = |sf: SymId| state_functions.contains(&sf);
    for state_function in &mut pb.context.state_functions {
        if sub(state_function.sym) {
            // remove the boolean return value, return value is now the last parameter
            state_function.tpe.pop();
        }
    }

    let sub_sv = |sv: &[SAtom]| match sv.first() {
        Some(x) => match SymId::try_from(*x) {
            Ok(sym) => sub(sym),
            _ => false,
        },
        None => false,
    };

    let is_true = |atom: Atom| match bool::try_from(atom) {
        Ok(value) => value,
        Err(_) => panic!("should not be reachable"),
    };

    let transform_chronicle = |ch: &mut Chronicle| {
        for cond in &mut ch.conditions {
            if sub_sv(&cond.state_var) {
                assert!(is_true(cond.value));
                let value = cond.state_var.pop().unwrap();
                cond.value = value.into();
            }
        }

        let mut i = 0;
        while i < ch.effects.len() {
            let eff = &mut ch.effects[i];
            if sub_sv(&eff.state_var) {
                if is_true(eff.value) {
                    let value = eff.state_var.pop().unwrap();
                    eff.value = value.into();
                    i += 1;
                } else {
                    ch.effects.remove(i);
                }
            } else {
                i += 1;
            }
        }
    };

    for instance in &mut pb.chronicles {
        transform_chronicle(&mut instance.chronicle);
    }
    for template in &mut pb.templates {
        transform_chronicle(&mut template.chronicle);
    }
}

#[allow(clippy::map_entry)]
fn substitutable(pb: &Problem, sf: &StateFun) -> bool {
    let model = &pb.context.model;
    // only keep boolean state functions
    match sf.tpe.last() {
        Some(Type::Bool) => (),
        _ => return false,
    }
    // only keep state variables with at least one parameter (last element of type is the return value)
    if sf.tpe.len() < 2 {
        return false;
    }
    // last parameter must be a symbol
    match sf.tpe[sf.tpe.len() - 2] {
        Type::Sym(_) => {}
        _ => return false,
    }

    let sf = TypedSym::new(sf.sym, model.symbols.type_of(sf.sym));

    let possibly_on_sf = |sv: &[SAtom]| match sv.first() {
        Some(x) => model.unifiable(*x, sf),
        _ => false,
    };

    let mut assignments = HashMap::new();
    for ch in &pb.chronicles {
        // check that we don't have more than one positive effect
        for eff in ch.chronicle.effects.iter().filter(|e| possibly_on_sf(&e.state_var)) {
            if let Some(e) = as_cst_eff(eff) {
                if e.value {
                    // positive assignment
                    let (sv, val) = e.into_assignment();
                    if assignments.contains_key(&sv) {
                        // more than one assignment, abort
                        return false;
                    } else {
                        assignments.insert(sv, val);
                    }
                }
            } else {
                // we have a possible static effect that contains non-constant, abort
                return false;
            }
        }

        // check that we have only positive conditions for this sv
        for cond in ch.chronicle.conditions.iter().filter(|c| possibly_on_sf(&c.state_var)) {
            if model.unifiable(cond.value, false) {
                return false;
            }
        }
    }
    for template in &pb.templates {
        // check that we have only positive conditions for this sv
        for cond in template
            .chronicle
            .conditions
            .iter()
            .filter(|c| possibly_on_sf(&c.state_var))
        {
            if model.unifiable(cond.value, false) {
                return false;
            }
        }

        for (k, group) in &template
            .chronicle
            .effects
            .iter()
            .filter(|e| possibly_on_sf(&e.state_var))
            .group_by(|e| &e.state_var[0..(e.state_var.len() - 1)])
        {
            if TypedSym::try_from(k[0]).ok() != Some(sf) {
                // not a constant state variable
                return false;
            }

            let group: Vec<_> = group.collect();
            let num_positive = group.iter().filter(|e| e.value == Atom::from(true)).count();
            let num_negative = group.iter().filter(|e| e.value == Atom::from(false)).count();
            // we must have exactly one positive and on negative effect
            if num_positive != 1 || num_negative != 1 || group.len() != 2 {
                return false;
            }
            let first = group[0];
            let second = group[1];
            // they must cover the same interval
            if first.persistence_start != second.persistence_start || first.transition_start != second.transition_start
            {
                return false;
            }
        }
    }

    // we have passed all the test, this predicate can be lifted as a state variable
    true
}

struct CstEff {
    sv: Vec<SymId>,
    value: bool,
}

impl CstEff {
    fn into_assignment(mut self) -> (Vec<SymId>, SymId) {
        assert!(self.value);
        let value = self.sv.pop().unwrap();
        (self.sv, value)
    }
}

fn as_cst_eff(eff: &Effect) -> Option<CstEff> {
    let mut c = CstEff {
        sv: vec![],
        value: false,
    };
    for x in &eff.state_var {
        c.sv.push(SymId::try_from(*x).ok()?)
    }
    c.value = bool::try_from(eff.value).ok()?;
    Some(c)
}
