use crate::chronicles::*;

use crate::chronicles::constraints::{Constraint, ConstraintType};
use crate::PRINT_PLANNER_OUTPUT;
use aries::model::extensions::{AssignmentExt, Shaped};
use aries::model::lang::{IAtom, SAtom};
use std::convert::TryFrom;
/// Detects state functions that are static (all of its state variable will take a single value over the entire planning window)
/// and replaces the corresponding conditions and effects as table constraints.
///
/// We are considering the state function is static if:
/// - it does not appears in template effects
/// - for effects on it in the chronicle instances,
///   - all variables (in the state variable and the value) must be defined
///   - the effect should start support at the time origin
pub fn statics_as_tables(pb: &mut Problem) {
    let context = &pb.context;

    // convenience functions
    let effect_is_static_assignment = |eff: &concrete::Effect| -> bool {
        // this effect is unifiable with our state variable, we can only make it static if all variables are bound
        if eff
            .state_var
            .args
            .iter()
            .any(|y| context.model.sym_domain_of(*y).size() != 1)
        {
            return false;
        }
        // effect must be an assignment of a constant
        if let EffectOp::Assign(value) = eff.operation {
            let (lb, ub) = context.model.int_bounds(value);
            if lb != ub {
                return false;
            }
        } else {
            return false;
        }
        eff.effective_start() == context.origin()
    };

    // Tables that will be added to the context at the end of the process (not done in the main loop to please the borrow checker)
    let mut additional_tables = Vec::new();

    let mut first = true;

    // process all state functions independently
    for target_fluent in &pb.context.fluents {
        // sf is the state function that we are evaluating for replacement.
        //  - first check that we are in fact allowed to replace it (it only has static effects and all conditions are convertible)
        //  - then transforms it: build a table with all effects and replace the conditions with table constraints
        let mut template_effects = pb.templates.iter().flat_map(|ch| &ch.chronicle.effects);

        let is_on_target_fluent = |state_var: &StateVar| target_fluent == &state_var.fluent;

        let appears_in_template_effects = template_effects.any(|eff| is_on_target_fluent(&eff.state_var));
        if appears_in_template_effects {
            continue; // not a static state function (appears in template)
        }

        let mut effects = pb.chronicles.iter().flat_map(|ch| ch.chronicle.effects.iter());

        let effects_init_and_bound = effects.all(|eff| {
            if is_on_target_fluent(&eff.state_var) {
                // this effect is unifiable with our state variable, we can only make it static if all variables are bound
                effect_is_static_assignment(eff)
            } else {
                true // not interesting, continue
            }
        });
        if !effects_init_and_bound {
            continue; // not a static state function (appears after INIT or not full defined)
        }

        // check that all conditions for this state variable can be converted to a table entry
        let chronicles = pb
            .templates
            .iter()
            .map(|tempplate| &tempplate.chronicle)
            .chain(pb.chronicles.iter().map(|ch| &ch.chronicle));
        let mut conditions = chronicles.flat_map(|ch| ch.conditions.iter());
        let conditions_ok = conditions.all(|cond| {
            if is_on_target_fluent(&cond.state_var) {
                // the value of this condition must be transformable to an int
                cond.value.int_view().is_some()
            } else {
                true // not interesting, continue
            }
        });
        if !conditions_ok {
            continue;
        }

        // === at this point, we know that the state function is static, we can replace all conditions/effects by a single constraint ===
        if first {
            if PRINT_PLANNER_OUTPUT.get() {
                println!("Transforming static state functions as table constraints:");
            }
            first = false;
        }
        let sf_name = pb.context.model.get_symbol(target_fluent.sym).to_string();

        if PRINT_PLANNER_OUTPUT.get() {
            println!(" - {sf_name}");
        }

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
                        let sym = SymId::try_from(*v).ok().unwrap();
                        line.push(sym.int_value());
                    }
                    let int_value = if let EffectOp::Assign(value) = e.operation {
                        let (lb, ub) = pb.context.model.int_bounds(value);
                        assert_eq!(lb, ub, "Not a constant");
                        lb
                    } else {
                        unreachable!("Not an assignment");
                    };

                    line.push(int_value);
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
                    let mut vars: Vec<IAtom> = c.state_var.args.iter().copied().map(SAtom::int_view).collect();
                    // add the value
                    vars.push(c.value.int_view().unwrap());
                    instance.chronicle.constraints.push(Constraint {
                        variables: vars.iter().map(|&i| Atom::from(i)).collect(),
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
                    let mut vars: Vec<IAtom> = c.state_var.args.iter().copied().map(SAtom::int_view).collect();

                    // add the value
                    vars.push(c.value.int_view().unwrap());
                    template.chronicle.constraints.push(Constraint {
                        variables: vars.iter().map(|&i| Atom::from(i)).collect(),
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
