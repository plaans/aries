use crate::chronicles::{Condition, Effect, Problem};

/// Remove all effects in the domain definition that are not unifiable with any condition.
pub fn remove_unusable_effects(pb: &mut Problem) {
    let model = &pb.context.model;

    // is the effect a possible support for this condition
    let possible_support = |e: &Effect, c: &Condition| -> bool {
        if c.state_var.len() != e.state_var.len() {
            return false;
        }
        for (ae, ac) in e.state_var.iter().zip(c.state_var.iter()) {
            if !model.unifiable(*ae, *ac) {
                return false;
            }
        }
        model.unifiable(e.value, c.value)
    };

    // returns true if the effect is unifiable with any condition (instance or template) in the problem
    let is_used = |e: &Effect, pb: &Problem| {
        for instance in &pb.chronicles {
            for c in &instance.chronicle.conditions {
                if possible_support(e, c) {
                    return true;
                }
            }
        }
        for template in &pb.templates {
            for c in &template.chronicle.conditions {
                if possible_support(e, c) {
                    return true;
                }
            }
        }
        false
    };

    let mut num_removed = 0;

    // loop on indices to please the borrow checker
    for instance_id in 0..pb.chronicles.len() {
        let mut i = 0;
        while i < pb.chronicles[instance_id].chronicle.effects.len() {
            let e = &pb.chronicles[instance_id].chronicle.effects[i];
            if !is_used(e, pb) {
                pb.chronicles[instance_id].chronicle.effects.remove(i);
                num_removed += 1;
            } else {
                i += 1
            }
        }
    }

    if num_removed > 0 {
        println!("Removed {} unusable effects", num_removed);
    }
}
