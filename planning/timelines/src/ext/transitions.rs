use std::collections::BTreeMap;

use aries::prelude::Lit;
use idmap::DirectIdMap;

use crate::constraints::HasValueAt;
use crate::encoder::{ConditionId, SchedEncoder};
use crate::{Effect, EffectId, IntTerm, StateVar, TaskId};

pub type SourceId = Option<TaskId>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Value is the index of a condition in a reference vector of constraints.
    Cond(ConditionId),
    /// Value is the index/identified of an effect in a collection of them.
    Eff(EffectId),
    /// Combination of Cond and Eff variants.
    CondEff(ConditionId, EffectId),
}
impl Transition {
    /*pub fn iter_groundings<'a>(
        &self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoderExt,
    ) -> impl StreamingIterator<Item = TransitionGrounding> {
        iter_transition_groundings(transition_id, ctx)
    }*/

    pub fn get_source(&self, ctx: &SchedEncoder) -> SourceId {
        match self {
            Transition::Cond(c_id) => ctx.causal_links.destinations[*c_id].source,
            Transition::Eff(e_id) => ctx.sched.effects[*e_id].source,
            Transition::CondEff(c_id, e_id) => {
                let res = ctx.causal_links.destinations[*c_id].source;
                debug_assert!(res == ctx.sched.effects[*e_id].source);
                res
            }
        }
    }
    pub fn get_prez(&self, ctx: &SchedEncoder) -> Lit {
        //pub fn get_prez(&self, ctx: &mut SchedEncoder) -> Lit {
        match self {
            Transition::Cond(c_id) => ctx.causal_links.destinations[*c_id].prez,
            Transition::Eff(e_id) => ctx.sched.effects[*e_id].prez,
            Transition::CondEff(c_id, e_id) => {
                let res = ctx.causal_links.destinations[*c_id].prez;
                debug_assert!(res == ctx.sched.effects[*e_id].prez);
                res
                //ctx.store.reify(aries::model::lang::expr::and([
                //    ctx.causal_links.destinations[*c_id].prez,
                //    ctx.sched.effects[*e_id].prez,
                //]))
            }
        }
    }
    pub fn get_state_var<'a>(&self, ctx: &'a SchedEncoder) -> &'a StateVar {
        match self {
            Transition::Cond(cid) => &ctx.causal_links.destinations[*cid].state_var,
            Transition::Eff(eid) => &ctx.sched.effects[*eid].state_var,
            Transition::CondEff(cid, eid) => {
                let res = &ctx.causal_links.destinations[*cid].state_var;
                debug_assert!(*res == ctx.sched.effects[*eid].state_var);
                res
            }
        }
    }
    pub fn get_terms<'a>(&self, ctx: &'a SchedEncoder) -> TransitionTerms<'a> {
        let args = match self {
            Transition::Cond(c_id) => &ctx.causal_links.destinations[*c_id].state_var.args,
            Transition::Eff(e_id) => &ctx.sched.effects[*e_id].state_var.args,
            Transition::CondEff(c_id, e_id) => {
                let c = &ctx.causal_links.destinations[*c_id];
                let e = &ctx.sched.effects[*e_id];
                debug_assert!(c.source.is_some());
                debug_assert!(c.source == e.source);
                debug_assert!(c.state_var == e.state_var);

                &c.state_var.args
            }
        };
        let (valfrom, valto) = match self {
            Transition::Cond(c_id) => (Some(ctx.causal_links.destinations[*c_id].value), None),
            Transition::Eff(e_id) => {
                let valto = match ctx.sched.effects[*e_id].operation {
                    crate::EffectOp::Assign(term) => term,
                    crate::EffectOp::Step(_) => todo!(),
                };
                (None, Some(valto))
            }
            Transition::CondEff(c_id, e_id) => {
                let c = &ctx.causal_links.destinations[*c_id];
                let e = &ctx.sched.effects[*e_id];
                debug_assert!(c.source.is_some());
                debug_assert!(c.source == e.source);
                debug_assert!(c.state_var == e.state_var);
                let valfrom = ctx.causal_links.destinations[*c_id].value;
                let valto = match ctx.sched.effects[*e_id].operation {
                    crate::EffectOp::Assign(term) => term,
                    crate::EffectOp::Step(_) => todo!(),
                };
                (Some(valfrom), Some(valto))
            }
        };
        debug_assert!(valfrom.is_some() || valto.is_some());

        TransitionTerms(args, valfrom, valto)
    }
}

pub struct TransitionTerms<'a>(&'a Vec<IntTerm>, Option<IntTerm>, Option<IntTerm>);

impl<'a> TransitionTerms<'a> {
    pub fn args(&self) -> &Vec<IntTerm> {
        self.0
    }
    pub fn valfrom(&self) -> Option<IntTerm> {
        self.1
    }
    pub fn valto(&self) -> Option<IntTerm> {
        self.2
    }
    pub fn unwrap(&self) -> (&Vec<IntTerm>, Option<IntTerm>, Option<IntTerm>) {
        (self.0, self.1, self.2)
    }
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len() + self.1.is_some() as usize + self.2.is_some() as usize
    }
    pub fn get(&self, i: usize) -> &IntTerm {
        if i == self.0.len() + 1 {
            self.2.as_ref().unwrap()
        } else if i == self.0.len() {
            self.1.as_ref().unwrap()
        } else {
            &self.0[i]
        }
    }
    pub fn iter(&'a self) -> impl Iterator<Item = &'a IntTerm> {
        self.0.iter().chain(self.1.iter()).chain(self.2.iter())
    }
    pub fn into_iter(&self) -> impl Iterator<Item = IntTerm> + use<'a> {
        self.0.iter().copied().chain(self.1).chain(self.2)
    }
}

pub type TransitionId = usize;

pub struct Transitions {
    pub store: Vec<Transition>,

    pub of_condition: DirectIdMap<ConditionId, TransitionId>,
    pub of_effect: DirectIdMap<EffectId, TransitionId>,
    of_empty_source: Vec<TransitionId>,
    of_concrete_source: DirectIdMap<TaskId, Vec<TransitionId>>,

    /// For each transition, stores the indices of its terms in its source's arguments
    /// (to avoid constantly recomputing them later).
    pub transition_terms_indices_in_source: Vec<Vec<usize>>,
}

pub fn get_source_terms(source_id: SourceId, ctx: &SchedEncoder) -> &[IntTerm] {
    if let Some(task_id) = source_id {
        &ctx.sched.tasks[task_id].args
    } else {
        &ctx.ext.as_ref().unwrap().empty_source_terms
    }
}

impl Transitions {
    pub fn iter(&self) -> impl Iterator<Item = ((TransitionId, Transition), SourceId)> {
        std::iter::chain(
            self.of_empty_source
                .iter()
                .map(|tr_id| ((*tr_id, self.store[*tr_id]), None)),
            self.of_concrete_source.iter().flat_map(move |(task_id, tr_ids)| {
                tr_ids
                    .iter()
                    .map(move |tr_id| ((*tr_id, self.store[*tr_id]), Some(task_id)))
            }),
        )
    }

    pub fn of_source(&self, source_id: &SourceId) -> Option<&Vec<TransitionId>> {
        match source_id {
            None => Some(&self.of_empty_source),
            Some(task_id) => self.of_concrete_source.get(task_id),
        }
    }

    pub fn from(ctx: &SchedEncoder, empty_source_terms: &Vec<IntTerm>) -> Self {
        let mut store = vec![];
        let mut of_empty_source = vec![];
        let mut of_concrete_source: DirectIdMap<TaskId, Vec<usize>> = DirectIdMap::default();
        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut transition_terms_indices_in_source = vec![];

        let mut add_transition =
            |tr: Transition,
             src_id: &SourceId,
             _of_condition: &mut DirectIdMap<ConditionId, TransitionId>,
             _of_effect: &mut DirectIdMap<EffectId, TransitionId>| {
                let tr_id = store.len();

                let src_terms = if src_id.is_none() {
                    empty_source_terms
                } else {
                    &ctx.sched.tasks[src_id.unwrap()].args
                };
                transition_terms_indices_in_source.push(
                    tr.get_terms(ctx)
                        .into_iter()
                        .map(|term| src_terms.iter().position(|&t| t == term).unwrap())
                        .collect(),
                );
                if src_id.is_none() {
                    of_empty_source.push(tr_id);
                } else if of_concrete_source.contains_key(src_id.unwrap()) {
                    of_concrete_source.get_mut(src_id.unwrap()).unwrap().push(tr_id);
                } else {
                    of_concrete_source.insert(src_id.unwrap(), vec![tr_id]);
                }
                match tr {
                    Transition::Cond(cid) => _of_condition.insert(cid, tr_id),
                    Transition::Eff(eid) => _of_effect.insert(eid, tr_id),
                    Transition::CondEff(cid, eid) => {
                        _of_condition.insert(cid, tr_id);
                        _of_effect.insert(eid, tr_id)
                    }
                };
                store.push(tr);
            };

        let conds_by_source = {
            let mut res = BTreeMap::<SourceId, Vec<(ConditionId, &HasValueAt)>>::new();
            for (cid, c) in ctx.causal_links.destinations.iter().enumerate() {
                res.entry(c.source).or_default().push((cid, c));
            }
            res
        };
        let effs_by_source = {
            let mut res = BTreeMap::<SourceId, Vec<(EffectId, &Effect)>>::new();
            for (eid, e) in ctx.sched.effects.iter().enumerate() {
                res.entry(e.source).or_default().push((eid, e));
            }
            res
        };

        // Search for CondEff transition patterns in each source
        // (except the empty one, in which this pattern is ignored).
        for (src_id, cs) in &conds_by_source {
            if src_id.is_some() && effs_by_source.contains_key(src_id) {
                for (eid, e) in effs_by_source.get(src_id).unwrap() {
                    for (cid, c) in cs {
                        debug_assert!(e.source == c.source);
                        if e.state_var == c.state_var && e.prez == c.prez {
                            add_transition(
                                Transition::CondEff(*cid, *eid),
                                src_id,
                                &mut of_condition,
                                &mut of_effect,
                            );
                        }
                    }
                }
            }
            // After all CondEff transitions in the currently considered source have
            // been found, cast the remaining conditions as simple Cond transitions.
            for (cid, _) in cs {
                if !of_condition.contains_key(cid) {
                    add_transition(Transition::Cond(*cid), src_id, &mut of_condition, &mut of_effect);
                }
            }
        }
        // Cast all effects that haven't been found to be part of a CondEff as simple Eff transitions
        for (src_id, es) in &effs_by_source {
            for (eid, _) in es {
                if !of_effect.contains_key(eid) {
                    add_transition(Transition::Eff(*eid), src_id, &mut of_condition, &mut of_effect);
                }
            }
        }

        Self {
            store,
            of_condition,
            of_effect,
            of_empty_source,
            of_concrete_source,
            transition_terms_indices_in_source,
        }
    }
}
