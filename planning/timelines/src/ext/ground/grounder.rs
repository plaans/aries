mod inner;
mod types;

use streaming_iterator::StreamingIterator;
use types::*;

use aries_solver::{core::views::Term, prelude::*};

use idmap::{DirectIdMap, intid::IntegerId};
use itertools::Itertools;

use crate::encoder::{CondId, SchedEncoder};
use crate::ext::{Source, collect_nonsimple_conditions_and_effects_to_relax, ground::SourceGrounding};
use crate::{Effect, EffectId, HasValueAt, Task, TaskId};

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
        let (conditions_to_ignore, effects_to_ignore) = collect_nonsimple_conditions_and_effects_to_relax(ctx);

        let global_args_groundings = ctx
            .sched
            .global_args
            .iter()
            .map(|t| ctx.sched.bounds(t).0..=ctx.sched.bounds(t).1)
            .multi_cartesian_product()
            .map(SourceGrounding::from)
            .collect();

        // all concrete sources by default
        let concrete_sources = ctx
            .sched
            .tasks
            .iter()
            .enumerate()
            .map(|(task_id, _)| TaskId::from_int(task_id as u32))
            .collect::<Vec<_>>();

        let mut program = GrounderProgram::new_empty();

        // Instead of "type facts", we have "domain facts" that are added for every variable's domain.
        // They are added in the the 3 functions below when encountering a variable.
        // And the hashset is a cache used to avoid repeatedly adding the same "domain facts".
        let mut cache_seen_doms = HashSet::<VarDom>::default();

        Self::add_initial_effects_facts(&mut program, &effects_to_ignore, ctx, &mut cache_seen_doms);
        Self::add_goal_rule(&mut program, &conditions_to_ignore, ctx, &mut cache_seen_doms);
        Self::add_all_actions_applicability_and_effects_rules(
            &mut program,
            &concrete_sources,
            (&conditions_to_ignore, &effects_to_ignore),
            ctx,
            &mut cache_seen_doms,
        );

        Self {
            program,
            global_args_groundings,
            concrete_sources,
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

    fn add_domain_fact_for_vardom(
        program: &mut GrounderProgram,
        vardom: VarDom,
        cache_seen_doms: &mut HashSet<VarDom>,
    ) {
        if cache_seen_doms.contains(&vardom) {
            return;
        }
        for c in vardom.0..=vardom.1 {
            program.add_fact((&GrounderPredicateId::Domain(vardom), [GrounderTerm::Cst(c)]));
        }
        cache_seen_doms.insert(vardom);
    }

    fn add_domain_facts_for_terms<'a>(
        program: &mut GrounderProgram,
        terms: impl Iterator<Item = &'a GrounderTerm>,
        ctx: &SchedEncoder,
        cache_seen_doms: &mut HashSet<VarDom>,
    ) {
        for term in terms {
            match term {
                GrounderTerm::Cst(_) => continue,
                GrounderTerm::Var(v) => {
                    let vardom = ctx.sched.bounds(v);
                    Self::add_domain_fact_for_vardom(program, vardom, cache_seen_doms);
                }
            }
        }
    }

    fn add_initial_effects_facts(
        program: &mut GrounderProgram,
        effects_to_ignore: &HashSet<EffectId>,
        ctx: &SchedEncoder,
        cache_seen_doms: &mut HashSet<VarDom>,
    ) {
        for (eff_id, eff) in ctx.sched.effects.iter().enumerate() {
            if eff.source.is_some() || effects_to_ignore.contains(&eff_id) {
                continue;
            }

            let Ok(terms) = collect_effect_datalog_terms(eff) else {
                unreachable!()
            };
            Self::add_domain_facts_for_terms(program, terms.iter(), ctx, cache_seen_doms);

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
                            let (lb, ub) = ctx.sched.bounds(*v);
                            lb..=ub
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

    fn add_goal_rule(
        program: &mut GrounderProgram,
        conditions_to_ignore: &HashSet<CondId>,
        ctx: &SchedEncoder,
        cache_seen_doms: &mut HashSet<VarDom>,
    ) {
        let goals = ctx
            .causal_links
            .conditions
            .iter()
            .enumerate()
            .filter(|(cond_id, c)| c.source.is_none() && !conditions_to_ignore.contains(cond_id));

        let mut goal_rule_body = vec![];

        for (_, goal) in goals.filter(|(cond_id, _)| !conditions_to_ignore.contains(cond_id)) {
            let Ok(terms) = collect_condition_datalog_terms(goal) else {
                unreachable!()
            };
            Self::add_domain_facts_for_terms(program, terms.iter(), ctx, cache_seen_doms);

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
        tasks: &[TaskId],
        conditions_and_effects_to_ignore: (&HashSet<CondId>, &HashSet<EffectId>),
        ctx: &SchedEncoder,
        cache_seen_doms: &mut HashSet<VarDom>,
    ) {
        let (conditions_to_ignore, effects_to_ignore) = conditions_and_effects_to_ignore;

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

        for &task_id in tasks {
            let (conditions, effects) = {
                let task_id = usize::try_from(task_id.to_int()).unwrap();

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
                (conditions, effects)
            };

            Self::add_action_applicability_and_effects_rules(
                program,
                (task_id, &ctx.sched.tasks[task_id]),
                conditions,
                effects,
                (conditions_to_ignore, effects_to_ignore),
                ctx,
                cache_seen_doms,
            )
        }
    }

    fn add_action_applicability_and_effects_rules(
        program: &mut GrounderProgram,
        task: (TaskId, &Task),
        conditions: &[(CondId, &HasValueAt)],
        effects: &[(EffectId, &Effect)],
        conditions_and_effects_to_ignore: (&HashSet<CondId>, &HashSet<EffectId>),
        ctx: &SchedEncoder,
        cache_seen_doms: &mut HashSet<VarDom>,
    ) {
        let (task_id, task) = task;
        let (conditions_to_ignore, effects_to_ignore) = conditions_and_effects_to_ignore;

        let applicability_rule_head = (
            &GrounderPredicateId::ActionApplicable(task_id),
            &task
                .args
                .iter()
                .filter_map(|t| {
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
            task.args
                .iter()
                .filter_map(|t| {
                    if t.is_cst() {
                        None
                    } else {
                        Some((GrounderTerm::Var(t.variable()), ctx.sched.bounds(t)))
                    }
                })
                .map(|(t, vardom)| {
                    Self::add_domain_fact_for_vardom(program, vardom, cache_seen_doms);

                    (GrounderPredicateId::Domain(vardom), vec![t])
                }),
        );
        applicability_rule_body.extend(
            conditions
                .iter()
                .filter(|(cond_id, _)| !conditions_to_ignore.contains(cond_id))
                .map(|(_, cond)| {
                    let Ok(terms) = collect_condition_datalog_terms(cond) else {
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
                let terms = collect_effect_datalog_terms(eff).unwrap();
                (&GrounderPredicateId::Fluent(eff.state_var.fluent.clone()), terms)
            };

            program.add_rule(effect_rule_head, &[applicability_rule_head]);
        }
    }
}

fn collect_condition_datalog_terms(cond: &HasValueAt) -> Result<Vec<GrounderTerm>, ()> {
    let terms = Vec::from_iter(cond.state_var.args.iter().copied().chain([cond.value]));

    Ok(Vec::from_iter(terms.into_iter().map(|term| {
        if term.is_cst() {
            GrounderTerm::Cst(term.constant)
        } else {
            GrounderTerm::Var(term.variable())
        }
    })))
}
fn collect_effect_datalog_terms(eff: &Effect) -> Result<Vec<GrounderTerm>, ()> {
    let crate::EffectOp::Assign(eff_value_term) = eff.operation else {
        return Err(());
    };

    let terms = Vec::from_iter(eff.state_var.args.iter().copied().chain([eff_value_term]));

    Ok(Vec::from_iter(terms.into_iter().map(|term| {
        if term.is_cst() {
            GrounderTerm::Cst(term.constant)
        } else {
            GrounderTerm::Var(term.variable())
        }
    })))
}
