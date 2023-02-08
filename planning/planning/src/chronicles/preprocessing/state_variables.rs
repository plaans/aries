use crate::chronicles::constraints::Constraint;
use crate::chronicles::{Chronicle, Container, Ctx, Effect, Problem, StateFun, VarType};
use aries_model::extensions::Shaped;
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
        println!(" - {}", pb.context.model.get_symbol(sf));
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

    // returns true if the corresponding state variable is one to be substituted
    let sub_sv = |sv: &[SAtom]| match sv.first() {
        Some(x) => match SymId::try_from(*x) {
            Ok(sym) => sub(sym),
            _ => false,
        },
        None => false,
    };

    let return_type_after_substitution = |sv: &[SAtom], context: &Ctx| {
        debug_assert!(sub_sv(sv));
        let x = sv.first().unwrap();
        let sym = SymId::try_from(*x).unwrap();
        let fluent = context.get_fluent(sym).unwrap();
        match fluent.return_type() {
            Type::Sym(type_id) => type_id,
            _ => unreachable!(""),
        }
    };

    let is_true = |atom: Atom| match bool::try_from(atom) {
        Ok(value) => value,
        Err(_) => unreachable!(),
    };

    let mut transform_chronicle = |ch: &mut Chronicle, container_label: Container| {
        // record all variables created in the process, they will need to me added to the chronicles
        // parameters afterwards
        let mut created_variables: Vec<Variable> = Default::default();
        for cond in &mut ch.conditions {
            if sub_sv(&cond.state_var) {
                match bool::try_from(cond.value) {
                    Ok(true) => {
                        // transform   [s,t] (loc r l) == true  into [s,t] loc r == l
                        let value = cond.state_var.pop().unwrap();
                        cond.value = value.into();
                    }
                    Ok(false) => {
                        // transform   [s,t] (loc r l) == false  into
                        //    [s,t] loc r == ?x    and      ?x != l
                        let forbidden_value = cond.state_var.pop().unwrap();
                        let var_type = return_type_after_substitution(&cond.state_var, &pb.context);
                        let var = pb.context.model.new_optional_sym_var(
                            var_type,
                            ch.presence,
                            container_label.var(VarType::Reification),
                        );
                        created_variables.push(var.into());
                        cond.value = var.into();
                        ch.constraints.push(Constraint::neq(var, forbidden_value));
                    }
                    Err(_) => unreachable!("State variable wrongly identified as substitutable"),
                }
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
        created_variables
    };

    for (id, instance) in pb.chronicles.iter_mut().enumerate() {
        let _created_vars = transform_chronicle(&mut instance.chronicle, Container::Instance(id));
        // no need to add the newly created variables to the parameters as it is not a template
        // and they need not be substituted
    }
    for (id, template) in pb.templates.iter_mut().enumerate() {
        let created_vars = transform_chronicle(&mut template.chronicle, Container::Template(id));
        // add new variables to the chronicle parameters, so they can be substituted upon instantiation of the template
        template.parameters.extend_from_slice(&created_vars);
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

    let sf = TypedSym::new(sf.sym, model.get_type_of(sf.sym));

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
                } else {
                    // negative assignment, not supported
                    return false;
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
        // check that we have only conditions with constant value for this sv
        for cond in template
            .chronicle
            .conditions
            .iter()
            .filter(|c| possibly_on_sf(&c.state_var))
        {
            if bool::try_from(cond.value).is_err() {
                return false; // not a constant value
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
                return false; // not a constant state variable
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
            // they must cover exactly the same interval
            if first.persistence_start != second.persistence_start
                || first.transition_start != second.transition_start
                || first.min_persistence_end != second.min_persistence_end
            {
                return false;
            }
        }
    }

    // we have passed all the tests, this predicate can be lifted as a state variable
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
