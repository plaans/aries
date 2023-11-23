use crate::chronicles::{Chronicle, EffectOp, Problem};

/// Preprocessing that identifies forced support where the start of a condition is equal to the start
/// of an effect on the same state variable. When this is the case, the condition is merged into the effect.
pub fn merge_conditions_effects(pb: &mut Problem) {
    let mut num_removed = 0;
    for ch in &mut pb.templates {
        process_chronicle(&mut ch.chronicle, &mut num_removed);
    }
    for ch in &mut pb.chronicles {
        process_chronicle(&mut ch.chronicle, &mut num_removed);
    }

    if num_removed > 0 {
        println!("Merged {num_removed} conditions into supporting effect.");
    }
}

fn process_chronicle(ch: &mut Chronicle, num_removed: &mut u32) {
    let mut i = 0;
    while i < ch.conditions.len() {
        let cond = &ch.conditions[i];
        for eff in &mut ch.effects {
            if cond.start == eff.transition_end
                && cond.state_var == eff.state_var
                && eff.operation == EffectOp::Assign(cond.value)
            {
                eff.min_mutex_end.push(cond.end);
                ch.conditions.remove(i);
                *num_removed += 1;
                i -= 1;
                break;
            }
        }
        i += 1;
    }
}
