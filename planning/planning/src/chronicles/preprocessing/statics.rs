use crate::chronicles::*;

use crate::chronicles::analysis::is_static;
use crate::chronicles::constraints::{Constraint, ConstraintType};
use aries::model::extensions::Shaped;
use std::convert::TryFrom;

fn is_on_fluent(target_fluent: &Fluent, state_var: &StateVar) -> bool {
    target_fluent == state_var.fluent.as_ref()
}

/// Detects state functions that are static (all of its state variable will take a single value over the entire planning window)
/// and replaces the corresponding conditions and effects as table constraints.
///
/// We are considering the state function is static if:
/// - it does not appears in template effects
/// - for effects on it in the chronicle instances,
///   - all variables (in the state variable and the value) must be defined
///   - the effect should start support at the time origin
pub fn statics_as_tables(pb: &mut Problem) {
    // Tables that will be added to the context at the end of the process (not done in the main loop to please the borrow checker)
    let mut additional_tables = Vec::new();

    let mut first = true;

    // process all state functions independently
    for target_fluent in &pb.context.fluents {
        let is_on_target_fluent = |state_var: &StateVar| is_on_fluent(target_fluent, state_var);

        if !is_static(target_fluent, pb) {
            continue;
        }

        // === at this point, we know that the state function is static, we can replace all conditions/effects by a single constraint ===
        if first {
            println!("Transforming static state functions as table constraints:");
            first = false;
        }
        let sf_name = pb.context.model.get_symbol(target_fluent.sym).to_string();
        println!(" - {sf_name}");

        // table that will collect all possible tuples for the state variable
        let mut table: Table<DiscreteValue> = Table::new(sf_name, target_fluent.signature.clone());

        // temporary buffer to work on before pushing to table
        let mut line = Vec::with_capacity(target_fluent.signature.len());

        // for each instance move all effects on `sf` to the table, and replace all conditions by a constraint
        for instance in &mut pb.chronicles {
            let mut i = 0;
            while i < instance.chronicle.effects.len() {
                let e = &instance.chronicle.effects[i];
                if is_on_target_fluent(&e.state_var) {
                    assert!(is_on_target_fluent(&e.state_var));
                    // we have an effect on this state variable
                    // create a new entry in the table
                    line.clear();
                    for v in &e.state_var.args {
                        let sym = TypedSym::try_from(*v).ok().unwrap();
                        line.push(DiscreteValue::Sym(sym));
                    }
                    let value = if let EffectOp::Assign(value) = e.operation {
                        DiscreteValue::try_from(value).expect("Not a value")
                    } else {
                        unreachable!("Not an assignment");
                    };

                    line.push(value);
                    table.push(&line);

                    // remove effect from chronicle
                    instance.chronicle.effects.remove(i);
                    continue; // skip increment
                }
                i += 1
            }
        }
        let table = Arc::new(table);

        for instance in &mut pb.chronicles {
            let mut i = 0;
            while i < instance.chronicle.conditions.len() {
                let c = &instance.chronicle.conditions[i];
                if is_on_target_fluent(&c.state_var) {
                    assert!(is_on_target_fluent(&c.state_var));
                    // debug_assert!(pb.context.domain(*x).as_singleton() == Some(sf.sym));
                    let c = instance.chronicle.conditions.remove(i);
                    // get variables from the condition's state variable
                    let mut vars: Vec<Atom> = c.state_var.args.iter().copied().map(Atom::from).collect();
                    // add the value
                    vars.push(c.value);
                    instance.chronicle.constraints.push(Constraint {
                        variables: vars,
                        tpe: ConstraintType::InTable(table.clone()),
                        value: None,
                    });

                    continue; // skip increment
                }
                i += 1;
            }
        }

        // for each template, replace all condition on the static state function by a table constraint
        for template in &mut pb.templates {
            let mut i = 0;
            while i < template.chronicle.conditions.len() {
                if is_on_target_fluent(&template.chronicle.conditions[i].state_var) {
                    let c = template.chronicle.conditions.remove(i);
                    assert!(is_on_target_fluent(&c.state_var));
                    // get variables from the condition's state variable
                    let mut vars: Vec<Atom> = c.state_var.args.iter().copied().map(Atom::from).collect();

                    // add the value
                    vars.push(c.value);
                    template.chronicle.constraints.push(Constraint {
                        variables: vars,
                        tpe: ConstraintType::InTable(table.clone()),
                        value: None,
                    });

                    continue; // skip increment, we already removed the current element
                }
                i += 1;
            }
        }

        additional_tables.push(table);
    }
}
