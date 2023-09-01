use crate::chronicles::constraints::Constraint;
use crate::chronicles::{Condition, Effect, Problem, VarLabel};
use crate::PRINT_PLANNER_OUTPUT;
use aries::model::lang::FAtom;
use aries::model::Model;
use std::cmp::Ordering;

// is the effect a possible support for this condition
fn is_possible_support(e: &Effect, c: &Condition, model: &Model<VarLabel>) -> bool {
    if c.state_var.len() != e.state_var.len() {
        return false;
    }
    for (ae, ac) in e.state_var.iter().zip(c.state_var.iter()) {
        if !model.unifiable(*ae, *ac) {
            return false;
        }
    }
    model.unifiable(e.value, c.value)
}

/// Returns true if the effect is unifiable with any condition (instance or template) in the problem
fn is_possibly_used(e: &Effect, pb: &Problem) -> bool {
    for instance in &pb.chronicles {
        for c in &instance.chronicle.conditions {
            if is_possible_support(e, c, &pb.context.model) {
                return true;
            }
        }
    }
    for template in &pb.templates {
        for c in &template.chronicle.conditions {
            if is_possible_support(e, c, &pb.context.model) {
                return true;
            }
        }
    }
    false
}

/// Remove all effects in the domain definition that are not unifiable with any condition.
pub fn remove_unusable_effects(pb: &mut Problem) {
    let mut num_removed = 0;

    // loop on indices to please the borrow checker
    for instance_id in 0..pb.chronicles.len() {
        let mut i = 0;
        while i < pb.chronicles[instance_id].chronicle.effects.len() {
            let e = &pb.chronicles[instance_id].chronicle.effects[i];
            if e.transition_start == e.persistence_start && e.min_persistence_end.is_empty() && !is_possibly_used(e, pb)
            {
                // effect is unused and instantaneous, it can be safely removed
                pb.chronicles[instance_id].chronicle.effects.remove(i);
                num_removed += 1;
            } else {
                i += 1
            }
        }
    }

    if num_removed > 0 && PRINT_PLANNER_OUTPUT.get() {
        println!("Removed {num_removed} unusable effects");
    }
}

/// Removes an effect that:
///  - is unused (cannot support any condition)
///  - ends exactly when another effect starts (more precisely, it persistence ends exactly when the
///    transition of the other effect starts.
///
/// Note that only effects in templates are processed currently but it could be extended to instances as well.
pub fn merge_unusable_effects(pb: &mut Problem) {
    let mut num_removed = 0;

    // loop on indices to please the borrow checker
    for instance_id in 0..pb.templates.len() {
        let mut i: isize = 0;
        while i < pb.templates[instance_id].chronicle.effects.len() as isize {
            let e = &pb.templates[instance_id].chronicle.effects[i as usize];
            if !is_possibly_used(e, pb) {
                // e cannot be used, find out if there is another effect in the chronicle that it can be merge into.
                for j in 0..pb.templates[instance_id].chronicle.effects.len() {
                    let e2 = &pb.templates[instance_id].chronicle.effects[j];
                    if i as usize == j || e.state_var != e2.state_var {
                        continue; // same effect or not on hte same state variable
                    }
                    if e2.transition_start == e.persistence_start
                        || e.min_persistence_end.contains(&e2.transition_start)
                    {
                        // the end of the persistence of `e` must meet the start of the transition of `e2`
                        // e: [ts1, te1] sv <- x
                        // e2 [ts2, te2] sv <- y  with te1 == ts2
                        // remove e and transform e2 into [ts1, te2] sv <- y
                        let e = e.clone();
                        let e2 = &mut pb.templates[instance_id].chronicle.effects[j];
                        let e2_old_start = e2.transition_start;
                        e2.transition_start = e.transition_start;
                        let ch = &mut pb.templates[instance_id].chronicle;

                        // when merging the effect, we should make sure that any constraint placed on the timepoints
                        // are still valid. Most of the time those will be tautological, so we explicitly check this to
                        // avoid overloading the chronicle

                        // lambda that enforces that `a <= b` in the chronicle
                        let mut enforce_leq = |a: FAtom, b: FAtom| match a.partial_cmp(&b) {
                            Some(Ordering::Equal | Ordering::Less) => { /* constraint is tautological, ignore */ }
                            None | Some(Ordering::Greater) => ch.constraints.push(Constraint::fleq(a, b)),
                        };

                        enforce_leq(e.transition_start, e.persistence_start);
                        enforce_leq(e.persistence_start, e2_old_start);
                        for e_min_persistence_end in e.min_persistence_end {
                            enforce_leq(e.persistence_start, e_min_persistence_end);
                            enforce_leq(e_min_persistence_end, e2_old_start);
                        }
                        // merging finished, remove the effect and update the counter
                        pb.templates[instance_id].chronicle.effects.remove(i as usize);
                        i -= 1;
                        num_removed += 1;
                        break;
                    }
                }
            }
            i += 1;
        }
    }

    if num_removed > 0 && PRINT_PLANNER_OUTPUT.get() {
        println!("Merged {num_removed} unusable effects");
    }
}
