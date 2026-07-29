mod inner;
mod types;

use streaming_iterator::StreamingIterator;
use types::*;

use aries_solver::{core::views::Term, prelude::*};

use idmap::{DirectIdMap, intid::IntegerId};
use itertools::Itertools;

use crate::constraints::HasValueAt;
use crate::encoder::{CondId, SchedEncoder};
use crate::ext::{Source, collect_nonsimple_conditions_and_effects_to_relax, ground::SourceGrounding};
use crate::{Effect, EffectId, TaskId};

use std::collections::{HashMap, HashSet};

// TODO: Consider individual action instances (taskid) only when not yet seen constant arguments assignments.
//       For the rest, just use the one corresponding to the fully lifted instance (if there's any, of course)

pub struct Grounder {
    program: GrounderProgram,
    global_args_groundings: Vec<SourceGrounding>,
    concrete_sources: Vec<TaskId>,
}

impl Grounder {
    /// WARNING: Assumes the causal links in the encoder to be already populated.
    ///          Indeed, the goals of the problem are otherwise inacessible and won't participate in the goal rule
    ///          (which will thus be a fact and result in even trivially inconsistent groundings to be computed).
    pub fn from(ctx: &SchedEncoder) -> Self {
        let (conditions_to_ignore, effects_to_ignore) = collect_conditions_and_effects_to_relax(ctx);

        let global_args_groundings = ctx
            .sched
            .global_args
            .iter()
            .map(|(t, _)| ctx.sched.bounds(t).0..=ctx.sched.bounds(t).1)
            .multi_cartesian_product()
            .map(SourceGrounding::from)
            .collect();

        let mut program = GrounderProgram::new_empty();

        Self::add_types_facts(&mut program, ctx);
        Self::add_initial_effects_facts(&mut program, &effects_to_ignore, ctx);
        Self::add_goal_rule(&mut program, &conditions_to_ignore, ctx);
        Self::add_all_actions_applicability_and_effects_rules(
            &mut program,
            &conditions_to_ignore,
            &effects_to_ignore,
            ctx,
        );

        Self {
            program,
            global_args_groundings,
            concrete_sources: ctx
                .sched
                .tasks
                .iter()
                .enumerate()
                .map(|(task_id, _)| TaskId::from_int(task_id as u32))
                .collect(),
        }
    }

    pub fn run(&self) -> HashMap<Source, Vec<SourceGrounding>> {
        // Build inner datalog program
        let mut inner = inner::GrounderProgramInner::new_empty();
        for fact in &self.program.facts {
            inner.add_fact(fact);
        }
        for rule in &self.program.rules {
            inner.add_rule(rule);
        }

        // Run (and consume) datalog program
        let inner_result = inner.run_and_consume();

        // Retrieve groundings
        let mut groundings = HashMap::default();

        for &task_id in &self.concrete_sources {
            groundings.insert(
                Some(task_id),
                inner_result.extract_groundings_of_concrete_source(task_id),
            );
        }
        // (add groundings "global args")
        groundings.insert(None, self.global_args_groundings.clone());

        groundings
    }

    pub fn print_datalog_program(&self) {
        self.program.print();
    }

    /// Adds facts specifying the type of each object (represented by its (unique) associated constant)
    fn add_types_facts(program: &mut GrounderProgram, ctx: &SchedEncoder) {
        for tpe in ctx.sched.objects.iter_types() {
            let grounder_predicate_id = GrounderPredicateId::Type(tpe.clone());

            let r = ctx.sched.objects.domain_of_type(tpe).unwrap();
            for c in r.first..=r.last {
                program.add_fact((&grounder_predicate_id, &[GrounderTerm::Cst(c)]));
            }
        }
    }

    fn add_initial_effects_facts(
        program: &mut GrounderProgram,
        effects_to_ignore: &HashSet<EffectId>,
        ctx: &SchedEncoder,
    ) {
        for (eff_id, eff) in ctx.sched.effects.iter().enumerate() {
            if eff.source.is_some() || effects_to_ignore.contains(&eff_id) {
                continue;
            }

            let Ok((terms, _)) = collect_effect_datalog_terms(eff, ctx) else {
                unreachable!()
            };

            if terms.iter().all(|t| matches!(t, GrounderTerm::Cst(_))) {
                // If all terms in the initial effect are constants (the effect is ground),
                // then add the corresponding fact directly.
                program.add_fact((&GrounderPredicateId::Fluent(eff.state_var.fluent.clone()), terms));
            } else {
                // If some terms in the initial effect are non-constant (variables),
                // then the relaxation is to consider all assignments / groundings of that effect as holding,
                // and we add the corresponding facts.
                let terms_bounds = terms
                    .iter()
                    .map(|t| match t {
                        GrounderTerm::Var(v) => {
                            let b = ctx.sched.bounds(*v);
                            b.0..=b.1
                        }
                        GrounderTerm::Cst(c) => *c..=*c,
                    })
                    .collect();
                let mut assignments = crate::boxes::enumeration::enumerate(terms_bounds);
                while let Some(terms) = assignments.next() {
                    let terms = Vec::from_iter(terms.iter().map(|&c| GrounderTerm::Cst(c)));
                    program.add_fact((&GrounderPredicateId::Fluent(eff.state_var.fluent.clone()), terms));
                }
            }
        }
    }

    fn add_goal_rule(program: &mut GrounderProgram, conditions_to_ignore: &HashSet<CondId>, ctx: &SchedEncoder) {
        let goals = ctx
            .causal_links
            .conditions
            .iter()
            .enumerate()
            .filter(|(cond_id, c)| c.source.is_none() && !conditions_to_ignore.contains(cond_id));

        let mut goal_rule_body = vec![];

        for (_, goal) in goals.filter(|(cond_id, _)| !conditions_to_ignore.contains(cond_id)) {
            let Ok(terms) = collect_condition_datalog_terms(goal, ctx) else {
                unreachable!()
            };
            goal_rule_body.push((GrounderPredicateId::Fluent(goal.state_var.fluent.clone()), terms));
        }

        if !goal_rule_body.is_empty() {
            let goal_rule_body = goal_rule_body
                .iter()
                .map(|(datalog_predicate_id, terms)| (datalog_predicate_id, terms.as_slice()))
                .collect::<Vec<_>>();
            program.add_rule((&GrounderPredicateId::Goal, &[]), &goal_rule_body);
        } else {
            program.add_fact((&GrounderPredicateId::Goal, &[]));
        }
    }

    fn add_all_actions_applicability_and_effects_rules(
        program: &mut GrounderProgram,
        conditions_to_ignore: &HashSet<CondId>,
        effects_to_ignore: &HashSet<EffectId>,
        ctx: &SchedEncoder,
    ) {
        let mut task_conditions = DirectIdMap::new();
        let mut task_effects = DirectIdMap::new();
        for (cond_id, c) in ctx.causal_links.conditions.iter().enumerate() {
            if let Some(task_id) = c.source.map(|task_id| task_id.to_int() as usize) {
                if conditions_to_ignore.contains(&cond_id) {
                    continue;
                }
                if !task_conditions.contains_key(task_id) {
                    task_conditions.insert(task_id, vec![]);
                }
                task_conditions[task_id].push((cond_id, c));
            }
        }
        for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
            if let Some(task_id) = e.source.map(|task_id| task_id.to_int() as usize) {
                if effects_to_ignore.contains(&eff_id) {
                    continue;
                }
                if !task_effects.contains_key(task_id) {
                    task_effects.insert(task_id, vec![]);
                }
                task_effects[task_id].push((eff_id, e));
            }
        }

        for (task_id, _) in ctx.sched.tasks.iter().enumerate() {
            let conditions = if task_conditions.contains_key(task_id) {
                task_conditions[task_id].as_slice()
            } else {
                [].as_slice()
            };
            let effects = if task_effects.contains_key(task_id) {
                task_effects[task_id].as_slice()
            } else {
                [].as_slice()
            };

            Self::add_action_applicability_and_effects_rules(
                program,
                TaskId::from_int(u32::try_from(task_id).unwrap()),
                conditions,
                effects,
                conditions_to_ignore,
                effects_to_ignore,
                ctx,
            )
        }
    }

    fn add_action_applicability_and_effects_rules(
        program: &mut GrounderProgram,
        task_id: TaskId,
        conditions: &[(CondId, &crate::constraints::HasValueAt)],
        effects: &[(EffectId, &crate::effects::Effect)],
        conditions_to_ignore: &HashSet<CondId>,
        effects_to_ignore: &HashSet<EffectId>,
        ctx: &SchedEncoder,
    ) {
        let applicability_rule_head = (
            &GrounderPredicateId::ActionApplicable(task_id),
            &ctx.sched.tasks[task_id]
                .args
                .iter()
                .filter_map(|(t, _)| {
                    if t.is_cst() {
                        None
                    } else {
                        Some(GrounderTerm::Var(t.variable()))
                    }
                })
                .collect::<Vec<_>>(),
        );

        let mut applicability_rule_body = vec![];

        applicability_rule_body.extend(
            ctx.sched.tasks[task_id]
                .args
                .iter()
                .filter_map(|(t, tpe)| {
                    if t.is_cst() {
                        None
                    } else {
                        Some((GrounderTerm::Var(t.variable()), tpe))
                    }
                })
                .map(|(t, tpe)| (GrounderPredicateId::Type(tpe.clone()), vec![t])),
        );
        applicability_rule_body.extend(
            conditions
                .iter()
                .filter(|(cond_id, _)| !conditions_to_ignore.contains(cond_id))
                .map(|(_, cond)| {
                    let Ok(terms) = collect_condition_datalog_terms(cond, ctx) else {
                        unreachable!()
                    };
                    (GrounderPredicateId::Fluent(cond.state_var.fluent.clone()), terms)
                }),
        );

        if !applicability_rule_body.is_empty() {
            let applicability_rule_body = applicability_rule_body
                .iter()
                .map(|(datalog_predicate_id, terms)| (datalog_predicate_id, terms.as_slice()))
                .collect::<Vec<_>>();
            program.add_rule(applicability_rule_head, &applicability_rule_body);
        } else {
            program.add_fact(applicability_rule_head);
        }

        for (_, eff) in effects.iter().filter(|(eff_id, _)| !effects_to_ignore.contains(eff_id)) {
            let effect_rule_head = {
                let (terms, negative) = collect_effect_datalog_terms(eff, ctx).unwrap();
                // if negative {
                //     continue;
                // }
                (&GrounderPredicateId::Fluent(eff.state_var.fluent.clone()), terms)
            };

            program.add_rule(effect_rule_head, &[applicability_rule_head]);
        }
    }
}

/// Corresponds to ambiguous conditions and effects + step effects and conditions they potentially support.
///
/// Alternative view: ignores (relaxes) conditions and effects over state variables
/// used in ambiguous or ill-defined conditions or effects.
fn collect_conditions_and_effects_to_relax(ctx: &SchedEncoder) -> (HashSet<CondId>, HashSet<EffectId>) {
    let (ambiguous_conditions, ambiguous_effects) = collect_nonsimple_conditions_and_effects_to_relax(ctx);

    let (mut conditions_to_ignore, mut effects_to_ignore) = (ambiguous_conditions, ambiguous_effects);

    for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
        match e.operation {
            crate::EffectOp::Assign(_) => (),
            crate::EffectOp::Step(_) => {
                effects_to_ignore.insert(eff_id);
            }
        }
    }
    for cl in ctx.causal_links.get_links() {
        if effects_to_ignore.contains(&cl.eff_id) {
            conditions_to_ignore.insert(cl.cond_id);
        }
    }

    (conditions_to_ignore, effects_to_ignore)
}

fn collect_condition_datalog_terms(cond: &HasValueAt, _ctx: &SchedEncoder) -> Result<Vec<GrounderTerm>, ()> {
    let terms = Vec::from_iter(cond.state_var.args.iter().copied().chain(
        // do not add effect value term if it corresponds to a boolean value
        [cond.value], //(!is_condition_boolean(cond, ctx).unwrap()).then_some(cond.value),
    ));

    Ok(Vec::from_iter(terms.into_iter().map(|term| {
        if term.is_cst() {
            GrounderTerm::Cst(term.constant)
        } else {
            GrounderTerm::Var(term.variable())
        }
    })))
}
fn collect_effect_datalog_terms(eff: &Effect, ctx: &SchedEncoder) -> Result<(Vec<GrounderTerm>, bool), ()> {
    let crate::EffectOp::Assign(eff_value_term) = eff.operation else {
        return Err(());
    };

    let negative = is_effect_boolean(eff, ctx).unwrap() && eff_value_term.is_cst() && eff_value_term.constant == 0;

    let terms = Vec::from_iter(eff.state_var.args.iter().copied().chain(
        // do not add effect value term if it corresponds to a boolean value
        [eff_value_term], //(!is_effect_boolean(eff, ctx).unwrap()).then_some(eff_value_term),
    ));

    Ok((
        Vec::from_iter(terms.into_iter().map(|term| {
            if term.is_cst() {
                GrounderTerm::Cst(term.constant)
            } else {
                GrounderTerm::Var(term.variable())
            }
        })),
        negative,
    ))
}

// fn is_condition_boolean(cond: &crate::HasValueAt, ctx: &SchedEncoder) -> Result<bool, ()> {
//     let cond_value_param = ctx.sched.fluents.get_return(&cond.state_var.fluent).unwrap();
//     Ok(!cond_value_param.is_sym_typed()) // TODO: change "!is_sym_typed" to "is_boolean_typed"
// }
fn is_effect_boolean(eff: &crate::Effect, ctx: &SchedEncoder) -> Result<bool, ()> {
    Ok(is_effect_boolean_positive(eff, ctx)? || is_effect_boolean_negative(eff, ctx)?)
}
fn is_effect_boolean_negative(eff: &crate::Effect, ctx: &SchedEncoder) -> Result<bool, ()> {
    let crate::EffectOp::Assign(eff_value_term) = eff.operation else {
        return Err(());
    };
    let eff_value_param = ctx.sched.fluents.get_return(&eff.state_var.fluent).unwrap();
    Ok(
        !eff_value_param.is_sym_typed() && eff_value_term.is_cst() && eff_value_term.constant == 0, // TODO: change "!is_sym_typed" to "is_boolean_typed"
    )
}
fn is_effect_boolean_positive(eff: &crate::Effect, ctx: &SchedEncoder) -> Result<bool, ()> {
    let crate::EffectOp::Assign(eff_value_term) = eff.operation else {
        return Err(());
    };
    let eff_value_param = ctx.sched.fluents.get_return(&eff.state_var.fluent).unwrap();
    Ok(
        !eff_value_param.is_sym_typed() && eff_value_term.is_cst() && eff_value_term.constant == 1, // TODO: change "!is_sym_typed" to "is_boolean_typed"
    )
}
