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
        println!("Merged {num_removed} conditions into supporting effect or identical condition.");
    }
}

fn process_chronicle(ch: &mut Chronicle, num_removed: &mut u32) {
    let mut i = 0;
    while i < ch.conditions.len() {
        let mut j = i + 1;
        while j < ch.conditions.len() {
            let condi = &ch.conditions[i];
            let condj = &ch.conditions[j];
            if condi.state_var == condj.state_var && condi.value == condj.value {
                // same condition, if they meet:
                // - expand the interval of the first to be the union of the two interval
                // - remove the second one
                if condi.end == condj.start {
                    let condj = ch.conditions.remove(j);
                    ch.conditions[i].end = condj.end;
                    *num_removed += 1;
                } else if condi.start == condj.end {
                    let condj = ch.conditions.remove(j);
                    ch.conditions[i].start = condj.start;
                    *num_removed += 1;
                } else {
                    j += 1;
                }
            }
        }
        i += 1
    }

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
