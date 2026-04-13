mod ground;

use std::collections::HashMap;

use aries::prelude::*;
use idmap::DirectIdMap;
use itertools::Itertools;

use crate::{ConditionId, Effect, EffectId, HasValueAt, SchedEncoder, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transition {
    /// Value is the index of a condition in a reference vector of constraints.
    Cond(ConditionId),
    /// Value is the index/identified of an effect in a collection of them.
    Eff(EffectId),
    /// Combination of Cond and Eff variants.
    CondEff(ConditionId, EffectId),
}
impl Transition {
    pub fn get_source(&self, ctx: &SchedEncoder) -> Option<TaskId> {
        let effects = &ctx.sched.effects;
        let conditions = &ctx.causal_links.destinations;

        match self {
            Transition::Cond(c_id) => conditions.get(*c_id).unwrap().source,
            Transition::Eff(e_id) => effects.get(*e_id).source,
            Transition::CondEff(c_id, e_id) => {
                debug_assert!(conditions.get(*c_id)?.source.is_some());
                debug_assert!(conditions.get(*c_id)?.source == effects.get(*e_id).source);
                conditions.get(*c_id).unwrap().source
            }
        }
    }
    pub fn get_prez(&self, ctx: &SchedEncoder) -> Lit {
        let effects = &ctx.sched.effects;
        let conditions = &ctx.causal_links.destinations;

        match self {
            Transition::Cond(c_id) => conditions.get(*c_id).unwrap().prez,
            Transition::Eff(e_id) => effects.get(*e_id).prez,
            Transition::CondEff(c_id, e_id) => {
                debug_assert!(conditions.get(*c_id).unwrap().source.is_some());
                debug_assert!(conditions.get(*c_id).unwrap().prez == effects.get(*e_id).prez);
                conditions.get(*c_id).unwrap().prez
            }
        }
    }
    pub fn get_args_and_vals<'a>(&self, ctx: &'a SchedEncoder) -> TransitionArgsAndVals<'a> {
        let effects = &ctx.sched.effects;
        let conditions = &ctx.causal_links.destinations;

        let args = match self {
            Transition::Cond(c_id) => &conditions.get(*c_id).unwrap().state_var.args,
            Transition::Eff(e_id) => &effects.get(*e_id).state_var.args,
            Transition::CondEff(c_id, e_id) => {
                let c = conditions.get(*c_id).unwrap();
                let e = effects.get(*e_id);
                debug_assert!(c.source.is_some());
                debug_assert!(c.source == e.source);
                debug_assert!(c.state_var == e.state_var);
                &c.state_var.args
            }
        };
        let (valfrom, valto) = match self {
            Transition::Cond(c_id) => (Some(conditions.get(*c_id).unwrap().value), None),
            Transition::Eff(e_id) => {
                let valto = match effects.get(*e_id).operation {
                    crate::EffectOp::Assign(linterm) => linterm,
                    crate::EffectOp::Step(linterm) => linterm,
                };
                (None, Some(valto))
            }
            Transition::CondEff(c_id, e_id) => {
                debug_assert!(conditions.get(*c_id).unwrap().source.is_some());
                debug_assert!(conditions.get(*c_id).unwrap().source == effects.get(*e_id).source);
                let valfrom = conditions.get(*c_id).unwrap().value;
                let valto = match effects.get(*e_id).operation {
                    crate::EffectOp::Assign(linterm) => linterm,
                    crate::EffectOp::Step(_) => todo!(),
                };
                (Some(valfrom), Some(valto))
            }
        };
        debug_assert!(valfrom.is_some() || valto.is_some());

        TransitionArgsAndVals(args, valfrom, valto)
    }
}

fn find_empty_source_linterms(ctx: &SchedEncoder) -> Vec<LinTerm> {
    let effects = &ctx.sched.effects;
    let conditions = &ctx.causal_links.destinations;

    std::iter::chain(
        effects
            .iter()
            .enumerate()
            .filter(|&(_eid, e)| e.source.is_none())
            .flat_map(|(eid, _e)| Transition::Eff(eid).get_args_and_vals(ctx).into_iter()),
        conditions
            .iter()
            .enumerate()
            .filter(|&(_cid, c)| c.source.is_none())
            .flat_map(|(cid, _c)| Transition::Cond(cid).get_args_and_vals(ctx).into_iter()),
    )
    .sorted()
    .unique()
    .collect_vec()

    /*BTreeSet::from_iter(
        effects.iter().enumerate().filter_map(|(eid, e)|
            e.source.is_none().then(|| Transition::Eff(eid).get_args_and_vals(ctx).iter())
        )
        .flatten()
        .chain(
            conditions.iter().enumerate().filter_map(|(cid, c)|
                c.source.is_none().then(|| Transition::Cond(cid).get_args_and_vals(ctx).iter())
            )
            .flatten()
        )
    )*/
}

pub struct TransitionArgsAndVals<'a>(&'a Vec<LinTerm>, Option<LinTerm>, Option<LinTerm>);

impl<'a> TransitionArgsAndVals<'a> {
    pub fn args(&self) -> &'a Vec<LinTerm> {
        self.0
    }
    pub fn valfrom(&self) -> Option<LinTerm> {
        self.1
    }
    pub fn valto(&self) -> Option<LinTerm> {
        self.2
    }
    pub fn arity(&self) -> usize {
        self.0.len() + self.1.is_some() as usize + self.2.is_some() as usize
    }
    pub fn iter(&'a self) -> impl Iterator<Item = &'a LinTerm> {
        self.0.iter().chain(self.1.iter()).chain(self.2.iter())
    }
    pub fn into_iter(&self) -> impl Iterator<Item = LinTerm> + use<'a> {
        self.0.iter().copied().chain(self.1).chain(self.2)
    }
    pub fn find_positions_in_source(
        &self,
        source: Option<TaskId>,
        ctx: &SchedEncoder,
        ctx_empty_source_linterms: &[LinTerm],
    ) -> (Vec<usize>, Option<usize>, Option<usize>) {
        let tasks = &ctx.sched.tasks;

        if let Some(task_id) = source {
            let args_pos = self
                .0
                .iter()
                .map(|&linterm| tasks[task_id].args.iter().position(|&lt| linterm == lt).unwrap())
                .collect_vec();
            let valfrom_pos = self
                .1
                .map(|linterm| tasks[task_id].args.iter().position(|&lt| linterm == lt).unwrap());
            let valto_pos = self
                .1
                .map(|linterm| tasks[task_id].args.iter().position(|&lt| linterm == lt).unwrap());
            (args_pos, valfrom_pos, valto_pos)
        } else {
            // TODO OPTIMIZATION: treat this case differently ! (empty_source_linterms may much larger than the args and vals).
            // Maybe by sorting first and then linear search ?
            let args_pos = self
                .0
                .iter()
                .map(|&linterm| ctx_empty_source_linterms.iter().position(|&lt| linterm == lt).unwrap())
                .collect_vec();
            let valfrom_pos = self
                .1
                .map(|linterm| ctx_empty_source_linterms.iter().position(|&lt| linterm == lt).unwrap());
            let valto_pos = self
                .1
                .map(|linterm| ctx_empty_source_linterms.iter().position(|&lt| linterm == lt).unwrap());
            (args_pos, valfrom_pos, valto_pos)
        }
    }
}

pub type TransitionId = usize;

pub struct Transitions {
    store: Vec<Transition>,
    of_empty_source: Vec<TransitionId>,
    of_concrete_source: DirectIdMap<TaskId, Vec<TransitionId>>,
    //of_effect: DirectIdMap<EffectId, TransitionId>,
    //of_condition: DirectIdMap<ConditionId, TransitionId>,
}
impl Transitions {
    pub fn of_source(&self, source: &Option<TaskId>) -> &Vec<TransitionId> {
        match source {
            None => &self.of_empty_source,
            Some(task_id) => &self.of_concrete_source[task_id],
        }
    }
    pub fn from(ctx: &SchedEncoder) -> Self {
        let mut store = vec![];
        let mut of_empty_source = vec![];
        let mut of_concrete_source: DirectIdMap<TaskId, Vec<TransitionId>> = DirectIdMap::default();
        let mut of_effect: DirectIdMap<EffectId, TransitionId> = DirectIdMap::default();
        let mut of_condition: DirectIdMap<ConditionId, TransitionId> = DirectIdMap::default();

        let effs_by_source = {
            let mut res = HashMap::<Option<TaskId>, Vec<(EffectId, &Effect)>>::new();
            for (eid, e) in ctx.sched.effects.iter().enumerate() {
                res.entry(e.source).or_default().push((eid, e));
            }
            res
        };
        let conds_by_source = {
            let mut res = HashMap::<Option<TaskId>, Vec<(ConditionId, &HasValueAt)>>::new();
            for (cid, c) in ctx.causal_links.destinations.iter().enumerate() {
                res.entry(c.source).or_default().push((cid, c));
            }
            res
        };

        for (src, cs) in &conds_by_source {
            if src.is_none() {
                continue;
            }
            while let Some(es) = effs_by_source.get(src) {
                for (cid, c) in cs {
                    for (eid, e) in es {
                        debug_assert!(e.source == c.source);
                        if e.state_var == c.state_var && e.prez == c.prez {
                            let tr = Transition::CondEff(*cid, *eid);
                            let tid = store.len();
                            store.push(tr);

                            of_concrete_source
                                .get_mut(src.unwrap())
                                .get_or_insert(&mut vec![])
                                .push(tid);
                            of_condition.insert(*cid, tid);
                            of_effect.insert(*eid, tid);
                        }
                    }
                }
            }
            for (cid, _) in cs {
                if of_condition.contains_key(cid) {
                    continue;
                }
                let tr = Transition::Cond(*cid);
                let tid = store.len();
                store.push(tr);

                if src.is_none() {
                    of_empty_source.push(tid);
                } else {
                    of_concrete_source
                        .get_mut(src.unwrap())
                        .get_or_insert(&mut vec![])
                        .push(tid);
                }
                of_condition.insert(*cid, tid);
            }
        }

        for (src, es) in &effs_by_source {
            for (eid, _) in es {
                if of_effect.contains_key(*eid) {
                    continue;
                }
                let tr = Transition::Eff(*eid);
                let tid = store.len();
                store.push(tr);

                if src.is_none() {
                    of_empty_source.push(tid);
                } else {
                    of_concrete_source
                        .get_mut(src.unwrap())
                        .get_or_insert(&mut vec![])
                        .push(tid);
                }
                of_effect.insert(*eid, tid);
            }
        }

        Self {
            store,
            of_empty_source,
            of_concrete_source,
            // of_effect,
            // of_condition,
        }
    }

    pub fn groundings_iterator<'a>(
        &'a self,
        ctx: &'a SchedEncoder,
    ) -> Result<ground::TransitionsGroundingsEnumerator<'a>, ()> {
        ground::TransitionsGroundingsEnumerator::new(self, ctx)
    }
}
