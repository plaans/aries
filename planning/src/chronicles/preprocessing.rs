use super::constraints::*;
use super::*;

use aries_model::assignments::Assignment;
use std::convert::{TryFrom, TryInto};

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

    // process all state functions independently
    for sf in &pb.context.state_functions {
        let mut template_effects = pb.templates.iter().flat_map(|ch| &ch.chronicle.effects);

        let appears_in_template_effects = template_effects.any(|eff| match eff.state_var.first() {
            Some(x) => unifiable(*x, sf.sym),
            _ => false,
        });
        if appears_in_template_effects {
            continue; // not a static state function (appears in template)
        }

        let mut effects = pb.chronicles.iter().flat_map(|ch| ch.chronicle.effects.iter());

        let effects_init_and_bound = effects.all(|eff| {
            match eff.state_var.first() {
                Some(x) if unifiable(*x, sf.sym) => {
                    // this effect is unifiable with our state variable, we can only make it static if all variables are bound
                    effect_is_static(eff)
                }
                _ => true, // not interesting, continue
            }
        });
        if !effects_init_and_bound {
            continue; // not a static state function (appears after INIT or not full defined)
        }

        // === at this point, we know that the state function is static, we can replace all conditions/effects by a single constraint ===

        // table that will collect all possible tuples for the state variable
        let mut table: Table<DiscreteValue> = Table::new(sf.tpe.clone());

        // temporary buffer to work on before pushing to table
        let mut line = Vec::with_capacity(sf.tpe.len());

        // future location of the table in the final problem (the table is not inserted right away to workaround the borrow checker)
        let table_id = (pb.context.tables.len() + additional_tables.len()) as u32;

        // for each instance move all effects on `sf` to the table, and replace all conditions by a constraint
        for instance in &mut pb.chronicles {
            let mut i = 0;
            while i < instance.chronicle.effects.len() {
                let e = &instance.chronicle.effects[i];
                if let Some(x) = e.state_var.first() {
                    if unifiable(*x, sf.sym) {
                        assert!(unified(*x, sf.sym));
                        // we have an effect on this state variable
                        // create a new entry in the table
                        line.clear();
                        for v in &e.state_var[1..] {
                            let sym = SymId::try_from(*v).ok().unwrap();
                            line.push(sym.int_value());
                        }
                        todo!()
                        // match e.value {
                        //     Atom::Bool(b) => {
                        //         if bool::try_from(b).ok().unwrap() {
                        //             1
                        //         } else {
                        //             0
                        //         }
                        //     }
                        //     Atom::Disc(d) => {
                        //         if let Ok(s) = SymId::try_from(d) {
                        //             s
                        //         }
                        //     }
                        // }
                        // line.push(context.domain(e.value).as_singleton().unwrap());
                        // table.push(&line);
                        //
                        // // remove effect from chronicle
                        // instance.chronicle.effects.remove(i);
                        // continue; // skip increment
                    }
                }
                i += 1
            }

            let mut i = 0;
            while i < instance.chronicle.conditions.len() {
                let e = &instance.chronicle.conditions[i];
                if let Some(x) = e.state_var.first() {
                    if unifiable(*x, sf.sym) {
                        assert!(unified(*x, sf.sym));
                        // debug_assert!(pb.context.domain(*x).as_singleton() == Some(sf.sym));
                        let c = instance.chronicle.conditions.remove(i);
                        // get variables from the condition's state variable
                        let mut vars: Vec<Atom> = c.state_var.iter().copied().map(Atom::from).collect();
                        // remove the state function
                        vars.remove(0);
                        // add the value
                        vars.push(c.value);
                        instance.chronicle.constraints.push(Constraint {
                            variables: vars,
                            tpe: ConstraintType::InTable { table_id },
                        });

                        continue; // skip increment
                    }
                }
                i += 1;
            }
        }

        // for each template, replace all condition on the static state function by a table constraint
        for template in &mut pb.templates {
            let mut i = 0;
            while i < template.chronicle.conditions.len() {
                if let Some(x) = template.chronicle.conditions[i].state_var.first() {
                    if unifiable(*x, sf.sym) {
                        assert!(unified(*x, sf.sym));
                        // debug_assert!(pb.context.domain(*x).as_singleton() == Some(sf.sym));
                        let c = template.chronicle.conditions.remove(i);
                        // get variables from the condition's state variable
                        let mut vars: Vec<Atom> = c.state_var.iter().copied().map(Atom::from).collect();
                        // remove the state function
                        vars.remove(0);
                        // add the value
                        vars.push(c.value);
                        template.chronicle.constraints.push(Constraint {
                            variables: vars,
                            tpe: ConstraintType::InTable { table_id },
                        });

                        continue; // skip increment, we already removed the current element
                    }
                }
                i += 1;
            }
        }

        additional_tables.push(table);
    }

    pb.context.tables.append(&mut additional_tables);
}
