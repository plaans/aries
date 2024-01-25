use crate::chronicles::constraints::Constraint;
use crate::chronicles::{Chronicle, Container, Effect, EffectOp, Fluent, Problem, StateVar, VarType};
use aries::model::extensions::Shaped;
use aries::model::lang::*;
use aries::model::symbols::SymId;
use itertools::Itertools;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use tracing;

/// Substitute predicates into state functions where applicable.
/// For instance the predicate `(at agent location) -> boolean` can usually be
/// transformed into the state function `(at agent) -> location`.
/// For this transformation to be applicable, it should be the case that,
/// for a given `agent`, there is at most one `location` such that `(at agent location) = true`.
pub fn predicates_as_state_variables(pb: &mut Problem) {
    let to_substitute: Vec<Arc<Fluent>> = pb
        .context
        .fluents
        .iter()
        .filter(|sf| {
            let name = pb.context.model.get_symbol(sf.sym).canonical_str();
            substitutable(pb, sf, name)
        })
        .cloned()
        .collect();
    if !to_substitute.is_empty() {
        println!("Substitution from predicate to state variable:")
    }
    for sf in &to_substitute {
        println!(" - {}", pb.context.model.get_symbol(sf.sym));
    }
    to_state_variables(pb, &to_substitute)
}

fn to_state_variables(pb: &mut Problem, fluents_to_lift: &[Arc<Fluent>]) {
    let mut trans: HashMap<SymId, Arc<Fluent>> = Default::default();
    for fluent in fluents_to_lift {
        let mut lifted = fluent.as_ref().clone();
        // remove the boolean return value, return value is now the last parameter
        lifted.signature.pop();
        trans.insert(lifted.sym, Arc::new(lifted));
    }
    let sub = |sf: &Fluent| trans.contains_key(&sf.sym);
    for fluent in &mut pb.context.fluents {
        if sub(fluent) {
            *fluent = trans[&fluent.sym].clone();
        }
    }

    // returns true if the corresponding state variable is one to be substituted
    let must_substitute = |sv: &Fluent| trans.contains_key(&sv.sym);

    let return_type = |sv: &Fluent| match sv.return_type() {
        Type::Sym(type_id) => type_id,
        _ => unreachable!(""),
    };

    let is_true_assignment = |eff: &EffectOp| eff == &EffectOp::TRUE_ASSIGNMENT;
    let is_false_assignment = |eff: &EffectOp| eff == &EffectOp::FALSE_ASSIGNMENT;

    let mut transform_chronicle = |ch: &mut Chronicle, container_label: Container| {
        // record all variables created in the process, they will need to me added to the chronicles
        // parameters afterwards
        let mut created_variables: Vec<Variable> = Default::default();
        for cond in &mut ch.conditions {
            if must_substitute(&cond.state_var.fluent) {
                // swap fluent with the lifted one
                cond.state_var.fluent = trans[&cond.state_var.fluent.sym].clone();
                match bool::try_from(cond.value) {
                    Ok(true) => {
                        // transform   [s,t] (loc r l) == true  into [s,t] loc r == l
                        let value = cond.state_var.args.pop().unwrap();
                        cond.value = value.into();
                    }
                    Ok(false) => {
                        // transform   [s,t] (loc r l) == false  into
                        //    [s,t] loc r == ?x    and      ?x != l
                        let forbidden_value = cond.state_var.args.pop().unwrap();
                        let var_type = return_type(&cond.state_var.fluent);
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
            if must_substitute(&eff.state_var.fluent) {
                // swap fluent witht he lifted one
                eff.state_var.fluent = trans[&eff.state_var.fluent.sym].clone();
                if is_true_assignment(&eff.operation) {
                    // transform `(at r l) := true` into  `(at r) := l`
                    let value = eff.state_var.args.pop().unwrap();
                    eff.operation = EffectOp::Assign(value.into());
                    i += 1;
                } else {
                    debug_assert!(is_false_assignment(&eff.operation));
                    // remove effects of the kind  `(at r l) := false`
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
fn substitutable(pb: &Problem, sf: &Fluent, fluent_name: &str) -> bool {
    let _span = tracing::span!(tracing::Level::TRACE, "to-sv", fluent = fluent_name).entered();
    let model = &pb.context.model;
    // only keep boolean state functions
    if sf.return_type() != Type::Bool {
        tracing::trace!("not bool");
        return false;
    }
    // only keep state variables with at least one parameter (last element of type is the return value)
    if sf.argument_types().is_empty() {
        tracing::trace!("no arguments");
        return false;
    }
    // last parameter must be a symbol
    match sf.argument_types().last().unwrap() {
        Type::Sym(_) => {}
        _ => {
            tracing::trace!("non sym return");
            return false;
        }
    }

    let on_target_fluent = |sv: &StateVar| sv.fluent.as_ref() == sf;

    let mut assignments = HashMap::new();
    for ch in &pb.chronicles {
        // check that we don't have more than one positive effect
        for eff in ch.chronicle.effects.iter().filter(|e| on_target_fluent(&e.state_var)) {
            if let Some(e) = as_cst_eff(eff) {
                if e.value {
                    // positive assignment
                    let (_, args, val) = e.into_assignment();
                    if assignments.contains_key(&args) {
                        // more than one assignment, abort
                        tracing::trace!("more than one positive assignment in base chronicles");
                        return false;
                    } else {
                        assignments.insert(args, val);
                    }
                } else {
                    // negative assignment, not supported
                    tracing::trace!("negative assignment in base chronicle");
                    return false;
                }
            } else {
                tracing::trace!("possible static effect with non-constant?");
                // we have a possible static effect that contains non-constant, abort
                return false;
            }
        }

        // check that we have only positive conditions for this sv
        for cond in ch
            .chronicle
            .conditions
            .iter()
            .filter(|c| on_target_fluent(&c.state_var))
        {
            if model.unifiable(cond.value, false) {
                // note that it is assumed that if an effect is present, it may be needed by someone
                // (there a special preprocessing phase that removes provably unused statements)
                tracing::trace!("non positive condition in base");
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
            .filter(|c| on_target_fluent(&c.state_var))
        {
            if bool::try_from(cond.value).is_err() {
                tracing::trace!("non constant condition in template");
                return false; // not a constant value
            }
        }

        for (_, group) in &template
            .chronicle
            .effects
            .iter()
            .filter(|e| on_target_fluent(&e.state_var))
            .group_by(|e| &e.state_var.args[0..(e.state_var.args.len() - 1)])
        {
            let group: Vec<_> = group.collect();
            let num_positive = group
                .iter()
                .filter(|e| e.operation == EffectOp::TRUE_ASSIGNMENT)
                .count();
            let num_negative = group
                .iter()
                .filter(|e| e.operation == EffectOp::FALSE_ASSIGNMENT)
                .count();
            // we must have exactly one positive and on negative effect
            if num_positive != 1 || num_negative != 1 || group.len() != 2 {
                tracing::trace!("not exactly one positive and one negative");
                return false;
            }
            let first = group[0];
            let second = group[1];
            // they must cover exactly the same interval
            if first.transition_end != second.transition_end
                || first.transition_start != second.transition_start
                || first.min_mutex_end != second.min_mutex_end
            {
                tracing::trace!("not covering the same interval");
                return false;
            }
        }
    }
    tracing::trace!("OK");
    // we have passed all the tests, this predicate can be lifted as a state variable
    true
}

struct CstEff {
    fluent: Arc<Fluent>,
    args: Vec<SymId>,
    value: bool,
}

impl CstEff {
    fn into_assignment(mut self) -> (Arc<Fluent>, Vec<SymId>, SymId) {
        assert!(self.value);
        let value = self.args.pop().unwrap();
        (self.fluent, self.args, value)
    }
}

fn as_cst_eff(eff: &Effect) -> Option<CstEff> {
    let mut c = CstEff {
        fluent: eff.state_var.fluent.clone(),
        args: vec![],
        value: false,
    };
    for x in &eff.state_var.args {
        c.args.push(SymId::try_from(*x).ok()?)
    }
    if let EffectOp::Assign(value) = eff.operation {
        c.value = bool::try_from(value).ok()?;
        Some(c)
    } else {
        None
    }
}
