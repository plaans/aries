use aries_solver::{core::views::Term, prelude::*};

use aries_datalog::{
    Arg as DatalogArg, Program as DatalogProgram, Rule as DatalogRule, Sym as DatalogSym, VarTable as DatalogPredicate,
};

use idmap::{DirectIdMap, intid::IntegerId};

use crate::{
    Effect, EffectId, Sym, TaskId,
    constraints::HasValueAt,
    encoder::{CondId, SchedEncoder},
    ext::{Source, collect_ambiguous_conditions_and_effects_to_relax, ground::SourceGrounding},
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SimpleDatalogGrounderPredicateId {
    Type(Sym),
    Fluent(Sym),
    ActionApplicable(TaskId),
    Goal,
}
impl std::fmt::Display for SimpleDatalogGrounderPredicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (head, tail) = match self {
            SimpleDatalogGrounderPredicateId::Type(s) => ("type".to_string(), Some(s)),
            SimpleDatalogGrounderPredicateId::Fluent(s) => ("fluent".to_string(), Some(s)),
            SimpleDatalogGrounderPredicateId::ActionApplicable(task_id) => (
                "action_applicable".to_string(),
                Some(&format!("some_{}", task_id.to_int())),
            ),
            SimpleDatalogGrounderPredicateId::Goal => ("goal".to_string(), None),
        };
        f.write_fmt(format_args!(
            "{head}{}",
            tail.map(|s| ["_", s].concat()).unwrap_or_default()
        ))
    }
}
#[derive(Debug, Clone)]
enum SimpleDatalogGrounderTerm {
    Var(Var),
    Cst(IntCst),
}
impl std::fmt::Display for SimpleDatalogGrounderTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Var(v) => f.write_fmt(format_args!("?{v:?}")),
            Self::Cst(c) => f.write_fmt(format_args!("{c}")),
        }
    }
}
#[derive(Debug, Clone)]
struct SimpleDatalogGrounderAtom {
    datalog_predicate_id: SimpleDatalogGrounderPredicateId,
    terms: Vec<SimpleDatalogGrounderTerm>,
}
impl std::fmt::Display for SimpleDatalogGrounderAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let terms = {
            let s = self.terms.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
            if !s.is_empty() {
                format!("({s})")
            } else {
                Default::default()
            }
        };
        f.write_fmt(format_args!("{}{terms}", self.datalog_predicate_id))
    }
}
#[derive(Debug, Clone)]
struct SimpleDatalogGrounderFact(SimpleDatalogGrounderAtom);
impl std::fmt::Display for SimpleDatalogGrounderFact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.", self.0))
    }
}
#[derive(Debug, Clone)]
struct SimpleDatalogGrounderRule {
    head: SimpleDatalogGrounderAtom,
    body: Vec<SimpleDatalogGrounderAtom>,
}
impl std::fmt::Display for SimpleDatalogGrounderRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{} :- {}.",
            self.head,
            self.body.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
        ))
    }
}

#[derive(Clone, Default)]
pub struct SimpleDatalogGrounderProgramView {
    facts: Vec<SimpleDatalogGrounderFact>,
    rules: Vec<SimpleDatalogGrounderRule>,
}
impl SimpleDatalogGrounderProgramView {
    pub fn print(&self) {
        for fact in &self.facts {
            println!("{fact}");
        }
        for rules in &self.rules {
            println!("{rules}");
        }
    }
}

// TODO: WHAT TO DO (IF ANYTHING) WHEN FACING VARIABLES IN GOAL OR INITIAL STATE ?
// TODO: Consider individual action instances (taskid) only when not yet seen constant arguments assignments.
//       For the rest, just use the one corresponding to the fully lifted instance (if there's any, of course)
pub struct SimpleDatalogGrounder {
    view: Option<SimpleDatalogGrounderProgramView>,
    inner: SimpleDatalogGrounderInner,

    global_args_groundings: Vec<SourceGrounding>,
}
impl SimpleDatalogGrounder {
    pub fn get_view(&self) -> Option<&SimpleDatalogGrounderProgramView> {
        self.view.as_ref()
    }
    pub fn clone_view(&self) -> Option<SimpleDatalogGrounderProgramView> {
        self.view.clone()
    }

    /// WARNING: Assumes the causal links in the encoder to be already populated.
    ///          Indeed, the goals of the problem are otherwise inacessible and won't participate in the goal rule
    ///          (which will thus be a fact and result in even trivially inconsistent groundings to be computed).
    pub fn from(ctx: &SchedEncoder, with_view: bool) -> Self {
        let (conditions_to_ignore, effects_to_ignore) = collect_conditions_and_effects_to_relax(ctx);

        use itertools::Itertools;
        let global_args_groundings = ctx
            .sched
            .global_args
            .iter()
            .map(|(t, _)| ctx.sched.bounds(t).0..=ctx.sched.bounds(t).1)
            .multi_cartesian_product()
            .map(SourceGrounding::from)
            .collect();

        let mut res = Self {
            view: with_view.then(SimpleDatalogGrounderProgramView::default),
            inner: SimpleDatalogGrounderInner::default(),
            global_args_groundings,
        };

        res.add_types_facts(ctx);
        res.add_initial_effects_facts(&effects_to_ignore, ctx);

        res.add_goal_rule(&conditions_to_ignore, ctx);

        res.add_all_actions_applicability_and_effects_rules(&conditions_to_ignore, &effects_to_ignore, ctx);

        res
    }
    pub fn run(self) -> HashMap<Source, Vec<SourceGrounding>> {
        let mut res = self.inner.run();
        res.insert(None, self.global_args_groundings);
        res
    }

    /// Adds facts specifying the type of each object (represented by its (unique) associated constant)
    fn add_types_facts(&mut self, ctx: &SchedEncoder) {
        for tpe in ctx.sched.objects.iter_types() {
            let datalog_predicate_id = SimpleDatalogGrounderPredicateId::Type(tpe.clone());

            let r = ctx.sched.objects.domain_of_type(tpe).unwrap();
            for c in r.first..=r.last {
                self.add_fact((&datalog_predicate_id, &[SimpleDatalogGrounderTerm::Cst(c)]));
            }
        }
    }
    fn add_initial_effects_facts(&mut self, effects_to_ignore: &HashSet<EffectId>, ctx: &SchedEncoder) {
        for (eff_id, eff) in ctx.sched.effects.iter().enumerate() {
            if eff.source.is_some() || effects_to_ignore.contains(&eff_id) {
                continue;
            }

            let Ok((terms, _)) = collect_effect_datalog_terms(eff, ctx) else {
                unreachable!()
            };

            if terms.iter().all(|t| matches!(t, SimpleDatalogGrounderTerm::Cst(_))) {
                self.add_fact((
                    &SimpleDatalogGrounderPredicateId::Fluent(eff.state_var.fluent.clone()),
                    terms,
                ));
            }
        }
    }
    fn add_goal_rule(&mut self, conditions_to_ignore: &HashSet<CondId>, ctx: &SchedEncoder) {
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
            goal_rule_body.push((
                SimpleDatalogGrounderPredicateId::Fluent(goal.state_var.fluent.clone()),
                terms,
            ));
        }

        if !goal_rule_body.is_empty() {
            let goal_rule_body = goal_rule_body
                .iter()
                .map(|(datalog_predicate_id, terms)| (datalog_predicate_id, terms.as_slice()))
                .collect::<Vec<_>>();
            self.add_rule((&SimpleDatalogGrounderPredicateId::Goal, &[]), &goal_rule_body);
        } else {
            self.add_fact((&SimpleDatalogGrounderPredicateId::Goal, &[]));
        }
    }
    fn add_all_actions_applicability_and_effects_rules(
        &mut self,
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

            self.add_action_applicability_and_effects_rules(
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
        &mut self,
        task_id: TaskId,
        conditions: &[(CondId, &crate::constraints::HasValueAt)],
        effects: &[(EffectId, &crate::effects::Effect)],
        conditions_to_ignore: &HashSet<CondId>,
        effects_to_ignore: &HashSet<EffectId>,
        ctx: &SchedEncoder,
    ) {
        let applicability_rule_head = (
            &SimpleDatalogGrounderPredicateId::ActionApplicable(task_id),
            &ctx.sched.tasks[task_id]
                .args
                .iter()
                .filter_map(|(t, _)| {
                    if t.is_cst() {
                        None
                    } else {
                        Some(SimpleDatalogGrounderTerm::Var(t.variable()))
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
                        Some((SimpleDatalogGrounderTerm::Var(t.variable()), tpe))
                    }
                })
                .map(|(t, tpe)| (SimpleDatalogGrounderPredicateId::Type(tpe.clone()), vec![t])),
        );
        applicability_rule_body.extend(
            conditions
                .iter()
                .filter(|(cond_id, _)| !conditions_to_ignore.contains(cond_id))
                .map(|(_, cond)| {
                    let Ok(terms) = collect_condition_datalog_terms(cond, ctx) else {
                        unreachable!()
                    };
                    (
                        SimpleDatalogGrounderPredicateId::Fluent(cond.state_var.fluent.clone()),
                        terms,
                    )
                }),
        );

        if !applicability_rule_body.is_empty() {
            let applicability_rule_body = applicability_rule_body
                .iter()
                .map(|(datalog_predicate_id, terms)| (datalog_predicate_id, terms.as_slice()))
                .collect::<Vec<_>>();
            self.add_rule(applicability_rule_head, &applicability_rule_body);
        } else {
            self.add_fact(applicability_rule_head);
        }

        for (_, eff) in effects.iter().filter(|(eff_id, _)| !effects_to_ignore.contains(eff_id)) {
            let effect_rule_head = {
                let (terms, negative) = collect_effect_datalog_terms(eff, ctx).unwrap();
                // if negative {
                //     continue;
                // }
                (
                    &SimpleDatalogGrounderPredicateId::Fluent(eff.state_var.fluent.clone()),
                    terms,
                )
            };

            self.add_rule(effect_rule_head, &[applicability_rule_head]);
        }
    }

    fn add_fact(
        &mut self,
        fact: (
            &SimpleDatalogGrounderPredicateId,
            impl AsRef<[SimpleDatalogGrounderTerm]>,
        ),
    ) {
        let (datalog_predicate_id, terms) = fact;

        if let Some(view) = self.view.as_mut() {
            view.facts.push(SimpleDatalogGrounderFact(SimpleDatalogGrounderAtom {
                datalog_predicate_id: datalog_predicate_id.clone(),
                terms: terms.as_ref().to_vec(),
            }));
        }
        self.inner.add_fact(datalog_predicate_id, terms);
    }

    fn add_rule(
        &mut self,
        head: (
            &SimpleDatalogGrounderPredicateId,
            impl AsRef<[SimpleDatalogGrounderTerm]>,
        ),
        body: &[(
            &SimpleDatalogGrounderPredicateId,
            impl AsRef<[SimpleDatalogGrounderTerm]>,
        )],
    ) {
        if let Some(view) = self.view.as_mut() {
            view.rules.push(SimpleDatalogGrounderRule {
                head: SimpleDatalogGrounderAtom {
                    datalog_predicate_id: head.0.clone(),
                    terms: head.1.as_ref().to_vec(),
                },
                body: body
                    .iter()
                    .map(|pair| SimpleDatalogGrounderAtom {
                        datalog_predicate_id: pair.0.clone(),
                        terms: pair.1.as_ref().to_vec(),
                    })
                    .collect::<Vec<_>>(),
            });
        }
        self.inner.add_rule(head, body);
    }
}

#[derive(Default)]
struct SimpleDatalogGrounderInner {
    prog: DatalogProgram,

    predicates: HashMap<SimpleDatalogGrounderPredicateId, usize>,

    datalog_sym_of_cst: HashMap<IntCst, u32>,
    cst_of_datalog_sym: DirectIdMap<u32, IntCst>,
    last_datalog_sym_of_cst: u32,
}

impl SimpleDatalogGrounderInner {
    fn add_fact(
        &mut self,
        datalog_predicate_id: &SimpleDatalogGrounderPredicateId,
        terms: impl AsRef<[SimpleDatalogGrounderTerm]>,
    ) {
        debug_assert!(
            terms
                .as_ref()
                .iter()
                .all(|t| matches!(t, SimpleDatalogGrounderTerm::Cst(_)))
        );

        let row = terms
            .as_ref()
            .iter()
            .map(|t| {
                if let SimpleDatalogGrounderTerm::Cst(c) = t {
                    self.get_or_intern_datalog_sym_of_cst(*c)
                } else {
                    unreachable!()
                }
            })
            .collect::<Vec<_>>();

        self.get_or_intern_datalog_predicate_mut(datalog_predicate_id, terms.as_ref().len())
            .add(row);
    }

    fn add_rule(
        &mut self,
        head: (
            &SimpleDatalogGrounderPredicateId,
            impl AsRef<[SimpleDatalogGrounderTerm]>,
        ),
        body: &[(
            &SimpleDatalogGrounderPredicateId,
            impl AsRef<[SimpleDatalogGrounderTerm]>,
        )],
    ) {
        let terms = head
            .1
            .as_ref()
            .iter()
            .map(|t| match t {
                SimpleDatalogGrounderTerm::Var(v) => DatalogArg::Var(v.to_u32()),
                SimpleDatalogGrounderTerm::Cst(c) => DatalogArg::Sym(self.get_or_intern_datalog_sym_of_cst(*c)),
            })
            .collect::<Vec<_>>();
        let head = self
            .get_or_intern_datalog_predicate(head.0, head.1.as_ref().len())
            .apply(terms);

        let body = body
            .iter()
            .map(|pair| {
                let terms = pair
                    .1
                    .as_ref()
                    .iter()
                    .map(|t| match t {
                        SimpleDatalogGrounderTerm::Var(v) => DatalogArg::Var(v.to_u32()),
                        SimpleDatalogGrounderTerm::Cst(c) => DatalogArg::Sym(self.get_or_intern_datalog_sym_of_cst(*c)),
                    })
                    .collect::<Vec<_>>();
                self.get_or_intern_datalog_predicate(pair.0, pair.1.as_ref().len())
                    .apply(terms)
            })
            .collect::<Vec<_>>();

        self.prog.add_rule(DatalogRule::new(head, body));
    }

    fn run(self) -> HashMap<Source, Vec<SourceGrounding>> {
        let var_tables = self.prog.run();

        let mut res = HashMap::default();
        for (predicate_id, predicate_index) in self.predicates {
            let SimpleDatalogGrounderPredicateId::ActionApplicable(task_id) = predicate_id else {
                continue;
            };
            let rows = var_tables[predicate_index]
                .extract()
                .rows()
                .map(|row| SourceGrounding::from(Vec::from_iter(row.iter().map(|&u| self.cst_of_datalog_sym[u]))))
                .collect::<Vec<_>>();
            res.insert(Some(task_id), rows);
        }
        res
    }

    fn get_or_intern_datalog_predicate(
        &mut self,
        predicate_id: &SimpleDatalogGrounderPredicateId,
        arity: usize,
    ) -> &DatalogPredicate {
        if !self.predicates.contains_key(predicate_id) {
            self.predicates.insert(predicate_id.clone(), self.prog.num_predicates());
            self.prog.new_predicate(arity);
        }
        let res = self.prog.get_predicate(self.predicates[predicate_id]).unwrap();
        assert!(res.arity() == arity);
        res
    }
    fn get_or_intern_datalog_predicate_mut(
        &mut self,
        predicate_id: &SimpleDatalogGrounderPredicateId,
        arity: usize,
    ) -> &mut DatalogPredicate {
        if !self.predicates.contains_key(predicate_id) {
            self.predicates.insert(predicate_id.clone(), self.prog.num_predicates());
            self.prog.new_predicate(arity);
        }
        let res = self.prog.get_predicate_mut(self.predicates[predicate_id]).unwrap();
        assert!(res.arity() == arity);
        res
    }
    fn get_or_intern_datalog_sym_of_cst(&mut self, cst: IntCst) -> DatalogSym {
        let res = *self
            .datalog_sym_of_cst
            .entry(cst)
            .or_insert(self.last_datalog_sym_of_cst);
        if !self.cst_of_datalog_sym.contains_key(res) {
            self.cst_of_datalog_sym.insert(res, cst);
            self.last_datalog_sym_of_cst += 1;
        }
        res
    }
}

/// Corresponds to ambiguous conditions and effects + step effects and conditions they potentially support.
///
/// Alternative view: ignores (relaxes) conditions and effects over state variables
/// used in ambiguous or ill-defined conditions or effects.
fn collect_conditions_and_effects_to_relax(ctx: &SchedEncoder) -> (HashSet<CondId>, HashSet<EffectId>) {
    let (ambiguous_conditions, ambiguous_effects) = collect_ambiguous_conditions_and_effects_to_relax(ctx);

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
        if conditions_to_ignore.contains(&cl.eff_id) {
            conditions_to_ignore.insert(cl.cond_id);
        }
    }

    (conditions_to_ignore, effects_to_ignore)
}

fn collect_condition_datalog_terms(
    cond: &HasValueAt,
    _ctx: &SchedEncoder,
) -> Result<Vec<SimpleDatalogGrounderTerm>, ()> {
    let terms = Vec::from_iter(cond.state_var.args.iter().copied().chain(
        // do not add effect value term if it corresponds to a boolean value
        [cond.value], //(!is_condition_boolean(cond, ctx).unwrap()).then_some(cond.value),
    ));

    Ok(Vec::from_iter(terms.into_iter().map(|term| {
        if term.is_cst() {
            SimpleDatalogGrounderTerm::Cst(term.constant)
        } else {
            SimpleDatalogGrounderTerm::Var(term.variable())
        }
    })))
}
fn collect_effect_datalog_terms(
    eff: &Effect,
    ctx: &SchedEncoder,
) -> Result<(Vec<SimpleDatalogGrounderTerm>, bool), ()> {
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
                SimpleDatalogGrounderTerm::Cst(term.constant)
            } else {
                SimpleDatalogGrounderTerm::Var(term.variable())
            }
        })),
        negative,
    ))
}

fn is_condition_boolean(cond: &crate::HasValueAt, ctx: &SchedEncoder) -> Result<bool, ()> {
    let cond_value_param = ctx.sched.fluents.get_return(&cond.state_var.fluent).unwrap();
    Ok(!cond_value_param.is_sym_typed()) // TODO: change "!is_sym_typed" to "is_boolean_typed"
}
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
