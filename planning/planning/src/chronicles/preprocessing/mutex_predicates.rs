use crate::chronicles::{Chronicle, Condition, Effect, EffectOp, Fluent, Problem};
use aries::model::extensions::Shaped;
use aries::model::lang::Atom;

#[derive(Default)]
struct Effects<'a> {
    pos: Vec<&'a Effect>,
    neg: Vec<&'a Effect>,
    other: Vec<&'a Effect>,
}

fn effects_on<'a>(fluent: &Fluent, chronicle: &'a Chronicle) -> Effects<'a> {
    let mut effs = Effects::default();
    for e in &chronicle.effects {
        if e.state_var.fluent.as_ref() == fluent {
            if e.operation == EffectOp::TRUE_ASSIGNMENT {
                effs.pos.push(e)
            } else if e.operation == EffectOp::FALSE_ASSIGNMENT {
                effs.neg.push(e)
            } else {
                effs.other.push(e)
            }
        }
    }
    effs
}
#[derive(Default)]
struct Conds<'a> {
    pos: Vec<&'a Condition>,
    neg: Vec<&'a Condition>,
    other: Vec<&'a Condition>,
}

fn conds_on<'a>(fluent: &Fluent, chronicle: &'a Chronicle) -> Conds<'a> {
    let mut conds = Conds::default();
    for e in &chronicle.conditions {
        if e.state_var.fluent.as_ref() == fluent {
            if e.value == Atom::TRUE {
                conds.pos.push(e)
            } else if e.value == Atom::FALSE {
                conds.neg.push(e)
            } else {
                conds.other.push(e)
            }
        }
    }
    conds
}

fn is_mutex_predicate(fluent: &Fluent, pb: &Problem) -> bool {
    let mut has_resource_pattern_usage = false;
    for ch in &pb.chronicles {
        let conds = conds_on(fluent, &ch.chronicle);

        if !conds.neg.is_empty() || !conds.other.is_empty() {
            return false;
        }
    }
    for ch in &pb.templates {
        let conds = conds_on(fluent, &ch.chronicle);
        let effs = effects_on(fluent, &ch.chronicle);

        if !conds.neg.is_empty() || !conds.other.is_empty() {
            // non positive conditions
            return false;
        }

        if !effs.other.is_empty() {
            return false;
        }

        if effs.pos.is_empty() && effs.neg.is_empty() {
            continue; // no effects in this chronicle, proceed to next
        }

        if effs.pos.len() != 1 || effs.neg.len() != 1 {
            return false; // not exactly on positive and one negative
        }

        // we have one positive, one negative. we must have one condition
        if conds.pos.len() != 1 {
            return false;
        }

        let cond = conds.pos[0];
        let neg_eff = effs.neg[0];
        let pos_eff = effs.pos[0];

        if cond.end != neg_eff.transition_start || neg_eff.transition_start != ch.chronicle.start {
            return false; // not the expected temporal pattern
        }

        if cond.state_var != neg_eff.state_var || neg_eff.state_var != pos_eff.state_var {
            return false; // not one the same state variables
        }

        has_resource_pattern_usage = true;
    }

    has_resource_pattern_usage
}

/// Detect mutex predicates whose only use is to lock a given resource, with no-one ever requiring it to be locked
/// A fluent is a mutex-predicate if:
///   - it is boolean
///   - it is never test to be false
///   - it always transitions to false and then to true in a single action
///   - before any delete effect, it is tested to be true
///
/// For instance an action with:
///  [start] f == true
///  [start, start+eps] f := false
///  [end, end+eps]   f := true
///
/// All usages of such fluents are replaced with a single durative effect [start, end+eps] := true
/// that prevents any usage in ]start, end]
pub fn preprocess_mutex_predicates(pb: &mut Problem) {
    let mut count = 0;
    for fluent in &pb.context.fluents {
        let fluent = fluent.as_ref();
        if is_mutex_predicate(fluent, pb) {
            if count == 0 {
                println!("Transforming fluents to resources")
            }
            count += 1;
            let name = pb.context.model.get_symbol(fluent.sym).canonical_str();
            println!(" - {name}");
            for ch in &mut pb.templates {
                let Some(neg_eff_id) =
                    ch.chronicle.effects.iter().position(|e| {
                        e.state_var.fluent.as_ref() == fluent && e.operation == EffectOp::FALSE_ASSIGNMENT
                    })
                else {
                    // no effect in this chronicle
                    continue;
                };

                let neg_eff = ch.chronicle.effects.remove(neg_eff_id);

                let pos_eff_id = ch
                    .chronicle
                    .effects
                    .iter()
                    .position(|e| e.state_var.fluent.as_ref() == fluent && e.operation == EffectOp::TRUE_ASSIGNMENT)
                    .expect("Negative effect without positive");
                let pos_eff = &mut ch.chronicle.effects[pos_eff_id];
                pos_eff.transition_start = neg_eff.transition_start
            }
        }
    }
}
