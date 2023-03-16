use crate::chronicles::*;

use crate::chronicles::constraints::{Constraint, ConstraintType};
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
    let effect_is_static = |eff: &concrete::Effect| -> bool {
        // this effect is unifiable with our state variable, we can only make it static if all variables are bound
        if eff
            .state_var
            .iter()
            .any(|y| context.model.sym_domain_of(*y).size() != 1)
        {
            return false;
        }
        eff.effective_start() == context.origin()
    };
    let unifiable = |var, sym| context.model.sym_domain_of(var).contains(sym);
    let unified = |var, sym| {
        let dom = context.model.sym_domain_of(var);
        dom.into_singleton() == Some(sym)
    };

    // Tables that will be added to the context at the end of the process (not done in the main loop to please the borrow checker)
    let mut additional_tables = Vec::new();

    let mut first = true;

    // process all state functions independently
    for sf in &pb.context.state_functions {
        // sf is the state function that we are evaluating for replacement.
        //  - first check that we are in fact allowed to replace it (it only has static effects and all conditions are convertible)
        //  - then transforms it: build a table with all effects and replace the conditions with table constraints
        let mut template_effects = pb.templates.iter().flat_map(|ch| &ch.chronicle.effects);

        let appears_in_template_effects = template_effects.any(|eff| match eff.state_var.first() {
            Some(x) => unifiable(*x, sf.sym),
            _ => false,
        });
        if appears_in_template_effects {
            continue; // not a static state function (appears in template)
        }

        // returns true if the given, state variable possibly matches the current state function
        let possibly_on_sf = |state_var: &Sv| matches!(state_var.first(), Some(x) if unifiable(*x, sf.sym));
        // returns true if the given, state variable definitely matches the current state function
        let on_sf = |state_var: &Sv| matches!(state_var.first(), Some(x) if unified(*x, sf.sym));

        let mut effects = pb.chronicles.iter().flat_map(|ch| ch.chronicle.effects.iter());

        let effects_init_and_bound = effects.all(|eff| {
            if possibly_on_sf(&eff.state_var) {
                // this effect is unifiable with our state variable, we can only make it static if all variables are bound
                effect_is_static(eff)
            } else {
                true // not interesting, continue
            }
        });
        if !effects_init_and_bound {
            continue; // not a static state function (appears after INIT or not full defined)
        }

        // checks that all conditions on this state function can be converted into a table constraint
        let conditions_convertible = {
            // all chronicles
            let chronicles = pb
                .templates
                .iter()
                .map(|ch| &ch.chronicle)
                .chain(pb.templates.iter().map(|ch| &ch.chronicle));
            // all conditions
            let conditions = chronicles.flat_map(|ch| ch.conditions.iter());
            let mut conditions_on_sf = conditions.filter(|c| possibly_on_sf(&c.state_var));
            conditions_on_sf.all(|c| c.value.int_view().is_some())
        };
        if !conditions_convertible {
            // at least one of the conditions can not be encoded with the table constraint, abort
            continue;
        }

        // === at this point, we know that the state function is static, we can replace all conditions/effects by a single constraint ===
        if first {
            println!("Transforming static state functions as table constraints:");
            first = false;
        }
        let sf_name = pb.context.model.get_symbol(sf.sym).to_string();
        println!(" - {sf_name}");

        // table that will collect all possible tuples for the state variable
        let mut table: Table<DiscreteValue> = Table::new(sf_name, sf.tpe.clone());

        // temporary buffer to work on before pushing to table
        let mut line = Vec::with_capacity(sf.tpe.len());

        // for each instance move all effects on `sf` to the table, and replace all conditions by a constraint
        for instance in &mut pb.chronicles {
            let mut i = 0;
            while i < instance.chronicle.effects.len() {
                let e = &instance.chronicle.effects[i];
                if possibly_on_sf(&e.state_var) {
                    assert!(on_sf(&e.state_var));
                    // we have an effect on this state variable
                    // create a new entry in the table
                    line.clear();
                    for v in &e.state_var[1..] {
                        let sym = SymId::try_from(*v).ok().unwrap();
                        line.push(sym.int_value());
                    }

                    let (lb, ub) = pb.context.model.int_bounds(e.value);
                    assert_eq!(lb, ub, "Not a constant");
                    let int_value = lb;

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
                if possibly_on_sf(&c.state_var) {
                    assert!(on_sf(&c.state_var));
                    // debug_assert!(pb.context.domain(*x).as_singleton() == Some(sf.sym));
                    let c = instance.chronicle.conditions.remove(i);
                    // get variables from the condition's state variable
                    let mut vars: Vec<IAtom> = c.state_var.iter().copied().map(SAtom::int_view).collect();
                    // remove the state function
                    vars.remove(0);
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
                if possibly_on_sf(&template.chronicle.conditions[i].state_var) {
                    let c = template.chronicle.conditions.remove(i);
                    assert!(on_sf(&c.state_var));
                    // get variables from the condition's state variable
                    let mut vars: Vec<IAtom> = c.state_var.iter().copied().map(SAtom::int_view).collect();
                    // remove the state function
                    vars.remove(0);
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
