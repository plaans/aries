use anyhow::{Context, Result};
use aries::core::*;
use aries::model::extensions::Shaped;
use aries::model::lang::{expr::*, Atom, Cst, FAtom, Type};
use aries_planning::chronicles::constraints::Table;
use aries_planning::chronicles::plan::ActionInstance;
use aries_planning::chronicles::*;
use itertools::Itertools;
use num_rational::Ratio;
use std::collections::BTreeMap;

use crate::Model;

use super::{instantiate, populate_with_template_instances};

/// For each action in the warm-up `plan`, appends a new chronicle instance into the `pb`.
pub fn populate_with_warm_up_plan(
    pb: &mut FiniteProblem,
    spec: &Problem,
    plan: &[ActionInstance],
    depth: u32,
) -> Result<()> {
    plan.iter()
        // Create one instance for each action in the plan
        .try_for_each(|action| {
            // Find the template that corresponds to the action
            let template_id = spec
                .templates
                .iter()
                .position(|t| format!("{}", t.label) == action.name)
                .context("Unknown action in warm-up plan")?;
            let template = &spec.templates[template_id];

            // Find the current number of instances of the action template
            let generation_id = pb
                .chronicles
                .iter()
                .filter(|c| match &c.origin {
                    ChronicleOrigin::FreeAction { template_id: id, .. } => *id == template_id,
                    _ => false,
                })
                .count();

            // Instantiate the action template
            let origin = ChronicleOrigin::FreeAction {
                template_id,
                generation_id,
            };
            let instance_id = pb.chronicles.len();
            let instance = instantiate(instance_id, template, origin, Lit::TRUE, Sub::empty(), pb)?;
            pb.chronicles.push(instance);
            Ok::<(), anyhow::Error>(())
        })
        // Create one instance for each depth of decomposition
        .and_then(|_| populate_with_template_instances(pb, spec, |_| Some(depth)))
}

/// Enforce some constraints to force `plan` to be the only solution of `pb`.
///
/// We make the assumption that the `pb` has been populated with `populate_with_warm_up_plan` and the same `plan`
/// so the order of the chronicles in `pb` is the same as the order of the actions in `plan`.
pub fn add_strict_same_plan_constraints(pb: &mut FiniteProblem, plan: &[ActionInstance]) -> Result<()> {
    debug_assert_eq!(pb.chronicles.len(), plan.len() + 1); // +1 for the initial chronicle
    plan.iter()
        .zip(pb.chronicles.iter().skip(1)) // Skip the initial chronicle
        .for_each(|(action, chronicle)| {
            // Force the presence of the chronicle
            pb.model.enforce(chronicle.chronicle.presence, []);

            // Bind the parameters of the chronicle with the action
            chronicle
                .chronicle
                .name
                .iter()
                .skip(1) // Skip the chronicle name (e.g., "move")
                .zip(action.params.iter())
                .for_each(|(var, val)| pb.model.enforce(eq(*var, *val), [chronicle.chronicle.presence]));

            // Bind the start time-points
            pb.model.enforce(
                eq(chronicle.chronicle.start, ratio_to_timepoint(action.start)),
                [chronicle.chronicle.presence],
            );

            // Bind the end time-points
            pb.model.enforce(
                eq(
                    chronicle.chronicle.end,
                    ratio_to_timepoint(action.start + action.duration),
                ),
                [chronicle.chronicle.presence],
            );
        });
    Ok(())
}

/// Enforce some constraints to force the solution of `pb` to be a subplan of `plan`.
///
/// We make the assumption that the `pb` has been populated with `populate_with_warm_up_plan` and the same `plan`
/// so the order of the chronicles in `pb` is the same as the order of the actions in `plan`.
pub fn add_flexible_same_plan_constraints(pb: &mut FiniteProblem, plan: &[ActionInstance]) -> Result<()> {
    debug_assert_eq!(pb.chronicles.len(), plan.len() + 1); // +1 for the initial chronicle

    // Retrieve the types used in the future table constraints
    // Each line of the table will be (action.start, ...action.params)
    let types = plan
        .iter()
        // Only keep the action name
        .map(|action| action.name.clone())
        // Group with chronicles
        .zip(pb.chronicles.iter().skip(1)) // Skip the initial chronicle
        // Remove duplicates, keep only one instance for each template
        .unique_by(|(action, _)| action.clone())
        // Get the parameter types
        .map(|(action, chronicle)| {
            let types = chronicle
                .chronicle
                .name
                .iter()
                .skip(1) // Skip the chronicle name (e.g., "move")
                .map(|p| pb.model.get_type(p.variable()).unwrap())
                .collect_vec();
            (action, types)
        })
        // Extend the types with the start time-point at the begining
        .map(|(action, types)| {
            let mut types = types;
            types.insert(0, Type::Int { lb: 0, ub: INT_CST_MAX });
            (action, types)
        })
        // Convert into a map
        .collect::<BTreeMap<_, _>>();

    // Create the tables for each template
    let tables = plan
        .iter()
        // Get the start time-point of the action and its parameters
        .map(|action| {
            let start = ratio_to_timepoint(action.start);
            let start = Ratio::new_raw(start.num.shift, start.denom);
            let mut params: Vec<Cst> = vec![start.into()];
            params.extend(action.params.iter().cloned());
            (action, params)
        })
        // Group by action name
        .fold(BTreeMap::<String, Vec<_>>::new(), |mut acc, (action, params)| {
            acc.entry(action.name.clone()).or_default().push(params);
            acc
        })
        // Create the table for each action template
        .into_iter()
        .map(|(action_name, params)| {
            let types = types.get(&action_name).unwrap();
            let init_table = Table::new(format!("{}_params", action_name), types.clone());
            let table = params.into_iter().fold(init_table, |mut table, params| {
                table.push(&params);
                table
            });
            (action_name, table)
        })
        .collect::<BTreeMap<_, _>>();

    // Force the presence of the chronicle
    pb.chronicles
        .iter()
        .skip(1) // Skip the initial chronicle
        .for_each(|chronicle| pb.model.enforce(chronicle.chronicle.presence, []));

    // Force the set of chronicles to cover the table of the corresponding action
    plan.iter()
        .zip(pb.chronicles.iter().skip(1)) // Skip the initial chronicle
        // Group by action name
        .fold(BTreeMap::<String, Vec<_>>::new(), |mut acc, (action, chronicle)| {
            acc.entry(action.name.clone()).or_default().push(chronicle);
            acc
        })
        .into_iter()
        .try_for_each(|(action_name, chronicles)| {
            let table = tables
                .get(&action_name)
                .context(format!("Cannot find the table of the action {}", action_name))?;
            let variables = chronicles
                .iter()
                .map(|chronicle| {
                    let mut params: Vec<Atom> = vec![chronicle.chronicle.start.into()];
                    params.extend(chronicle.chronicle.name.iter().skip(1).cloned());
                    params
                })
                .collect_vec();
            let presences = chronicles
                .iter()
                .map(|chronicle| chronicle.chronicle.presence)
                .collect_vec();
            enforce_cover_table(&mut pb.model, variables, table, presences);

            Ok::<(), anyhow::Error>(())
        })?;

    Ok(())
}

/// Enforce the list of variables to match a line of the table.
///
/// Returns the literals that are true iff the variables are matching the line.
fn enforce_in_table(model: &mut Model, variables: Vec<Atom>, table: &Table<Cst>, presence: Lit) -> Vec<Lit> {
    // Returns a conjunction of literals that are true iff the variable is equal to the value
    let equals = |var: Atom, val: Cst, model: &mut Model| match var {
        Atom::Bool(lit) => {
            let Cst::Bool(val) = val else { unreachable!() };
            vec![if val { lit } else { !lit }]
        }
        Atom::Int(iatom) => {
            let Cst::Int(val) = val else { unreachable!() };
            vec![model.reify(leq(iatom, val)), model.reify(geq(iatom, val))]
        }
        Atom::Fixed(fatom) => {
            let Cst::Fixed(val) = val else { unreachable!() };
            vec![model.reify(f_leq(fatom, val)), model.reify(f_geq(fatom, val))]
        }
        Atom::Sym(satom) => {
            let Cst::Sym(val) = val else { unreachable!() };
            vec![model.reify(eq(satom, val))]
        }
    };
    let reif_equals = |var: Atom, val: Cst, model: &mut Model| {
        let lits = equals(var, val, model);
        if lits.len() == 1 {
            lits[0]
        } else {
            model.reify(and(lits))
        }
    };

    // Force to match at least one line
    let match_line = table
        .lines()
        // Check that the table line has the same number of values as the variables
        .inspect(|line| {
            debug_assert_eq!(line.len(), variables.len());
        })
        // For each table line, create a literal that is true iff the variables are matching the line
        .map(|line| {
            variables
                .iter()
                .zip(line.iter())
                .flat_map(|(&var, &val)| equals(var, val, model))
                .collect_vec()
        })
        .collect_vec() // collect to require unique access to `*model` at the same time
        .into_iter()
        .map(|supported_by_line| model.reify(and(supported_by_line)))
        .collect_vec();
    model.enforce(or(match_line.clone()), [presence]);

    // Reduce the domain of each variable to the values in the table
    variables
        .iter()
        .zip(table.columns())
        .map(|(&var, column)| (var, column.into_iter().unique().sorted().collect_vec()))
        .map(|(var, column)| {
            column
                .into_iter()
                .map(|&val| reif_equals(var, val, model))
                .collect_vec()
        })
        .collect_vec() // collect to require unique access to `*model` at the same time
        .into_iter()
        .for_each(|allowed_values| model.enforce(or(allowed_values), [presence]));

    // Add a redundant constraint such that the variable is either supported by the line or not match the value
    let support_table = match_line.iter().zip(table.lines()).collect_vec();
    variables
        .iter()
        .enumerate()
        .zip(table.columns())
        .map(|(var, column)| (var, column.into_iter().unique().sorted().collect_vec()))
        .for_each(|((idx, &var), column)| {
            let support_column = support_table.iter().map(|(lit, line)| (lit, line[idx])).collect_vec();
            match var {
                Atom::Bool(_) => unimplemented!(),
                Atom::Int(var) => {
                    column
                        .into_iter()
                        .map(|&val| match val {
                            Cst::Int(n) => n,
                            _ => panic!(),
                        })
                        .for_each(|n| {
                            let mut ge_clause = vec![!var.ge_lit(n)];
                            let mut le_clause = vec![!var.le_lit(n)];
                            support_column
                                .iter()
                                .map(|(&&lit, val)| match val {
                                    Cst::Int(n) => (lit, n),
                                    _ => panic!(),
                                })
                                .for_each(|(lit, &val)| {
                                    if val >= n {
                                        ge_clause.push(lit);
                                    }
                                    if val <= n {
                                        le_clause.push(lit);
                                    }
                                });
                            model.enforce(or(ge_clause), [presence]);
                            model.enforce(or(le_clause), [presence]);
                        });
                }
                Atom::Fixed(var) => {
                    column
                        .into_iter()
                        .map(|&val| match val {
                            Cst::Fixed(f) => f,
                            _ => panic!(),
                        })
                        .for_each(|f| {
                            let mut ge_clause = vec![!var.num.ge_lit(f.numer() * var.denom / f.denom())];
                            let mut le_clause = vec![!var.num.le_lit(f.numer() * var.denom / f.denom())];
                            support_column
                                .iter()
                                .map(|(&&lit, val)| match val {
                                    Cst::Fixed(f) => (lit, f),
                                    _ => panic!(),
                                })
                                .for_each(|(lit, &val)| {
                                    if val >= f {
                                        ge_clause.push(lit);
                                    }
                                    if val <= f {
                                        le_clause.push(lit);
                                    }
                                });
                            model.enforce(or(ge_clause), [presence]);
                            model.enforce(or(le_clause), [presence]);
                        });
                }
                Atom::Sym(var) => {
                    column.into_iter().for_each(|&val| {
                        let mut clause = vec![!reif_equals(var.into(), val, model)];
                        support_column
                            .iter()
                            .filter(|(_, v)| *v == val)
                            .for_each(|(&&lit, _)| clause.push(lit));
                        model.enforce(or(clause), [presence]);
                    });
                }
            }
        });

    match_line
}

/// Enforce the list of variables to cover the table.
fn enforce_cover_table(model: &mut Model, variables: Vec<Vec<Atom>>, table: &Table<Cst>, presences: Vec<Lit>) {
    // Force each line of the variables to match exactly one line of the table
    let var_line_match_tab_line = variables
        .iter()
        .zip(presences.iter())
        // Force to match at least one line
        .map(|(params, &presence)| (enforce_in_table(model, params.clone(), table, presence), presence))
        .collect_vec() // collect to require unique access to `*model` at the same time
        .iter()
        .inspect(|(lits, presence)| {
            // Force the unicity
            lits.iter()
                .combinations(2)
                .map(|pair| (pair[0], pair[1]))
                .for_each(|(&a, &b)| model.enforce(or([!a, !b]), [*presence]));
        })
        .map(|(lits, _)| lits.clone())
        .collect_vec();

    // Force each line of the table to match exactly one line of the variables
    let tab_line_match_var_line = transpose(var_line_match_tab_line);
    tab_line_match_var_line.into_iter().for_each(|lits| {
        // Force to match at least one line
        model.enforce(or(lits.clone()), presences.iter().copied());
        // Force the unicity
        lits.iter()
            .zip(presences.iter())
            .combinations(2)
            .map(|pair| (pair[0], pair[1]))
            .for_each(|((&a, &presence_a), (&b, &presence_b))| model.enforce(or([!a, !b]), [presence_a, presence_b]));
    });
}

fn transpose<T: Clone>(original: Vec<Vec<T>>) -> Vec<Vec<T>> {
    let rows = original.len();
    let cols = original.iter().map(Vec::len).max().unwrap_or(0);
    assert!(original.iter().all(|row| row.len() == cols));
    if rows == 0 || cols == 0 {
        return original;
    }
    (0..cols)
        .map(|col| (0..rows).map(|row| original[row][col].clone()).collect())
        .collect()
}

fn ratio_to_timepoint(ratio: Ratio<IntCst>) -> FAtom {
    let factor = TIME_SCALE.get() / ratio.denom();
    debug_assert_eq!(factor * ratio.denom(), TIME_SCALE.get());
    let numer = ratio.numer() * factor;
    let denom = ratio.denom() * factor;
    FAtom::new(numer.into(), denom)
}
