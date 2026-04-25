use std::collections::BTreeMap;

use aries::prelude::Lit;
use idmap::DirectIdMap;
use smallvec::{SmallVec, smallvec};

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

pub struct TransitionWithCtx<'a>(&'a Transition, &'a SchedEncoder);
impl<'a> std::fmt::Debug for TransitionWithCtx<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut _str = String::new();
        write!(
            f,
            "({:?}, {:?}) {:?}: {}",
            self.0,
            self.0.get_source_id(self.1),
            self.0.get_state_var(self.1),
            {
                let terms = self.0.get_terms(self.1);
                _str = format!("{:?}->{:?}", terms.valfrom(), terms.valto());
                &_str
            }
        )?;
        Ok(())
    }
}
impl<'a> TransitionWithCtx<'a> {
    /*pub fn iter_groundings(
        &self,
        transition_id: TransitionId,
    ) -> impl aries::utils::StreamingIterator<Item = crate::ext::TransitionGrounding> {
        self.0.iter_groundings(transition_id, self.1)
    }*/
    pub fn get_condition(&self) -> Option<&'a HasValueAt> {
        self.0.get_condition(self.1)
    }
    pub fn get_effect(&self) -> Option<&'a Effect> {
        self.0.get_effect(self.1)
    }
    pub fn get_source_id(&self) -> SourceId {
        self.0.get_source_id(self.1)
    }
    pub fn get_prezs(&self) -> Lit {
        self.0.get_prez(self.1)
    }
    /*pub fn get_prezs(&self) -> (Option<Lit>, Option<Lit>) {
        self.0.get_prezs(self.1)
    }*/
    pub fn get_state_var(&self) -> &'a StateVar {
        self.0.get_state_var(self.1)
    }
    pub fn get_terms(&self) -> TransitionTerms<'a> {
        self.0.get_terms(self.1)
    }
}

impl Transition {
    pub fn with_ctx<'a>(&'a self, ctx: &'a SchedEncoder) -> TransitionWithCtx<'a> {
        TransitionWithCtx(self, ctx)
    }
    /*pub fn iter_groundings<'a>(
        &self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> impl aries::utils::StreamingIterator<Item = crate::ext::TransitionGrounding> {
        crate::ext::ground::iter_transition_groundings(transition_id, ctx)
    }*/

    pub fn get_condition<'a>(&self, ctx: &'a SchedEncoder) -> Option<&'a HasValueAt> {
        match self {
            Transition::Eff(_) => None,
            Transition::Cond(c_id) | Transition::CondEff(c_id, _) => Some(&ctx.causal_links.destinations[*c_id]),
        }
    }

    pub fn get_effect<'a>(&self, ctx: &'a SchedEncoder) -> Option<&'a Effect> {
        match self {
            Transition::Cond(_) => None,
            Transition::Eff(e_id) | Transition::CondEff(_, e_id) => Some(&ctx.sched.effects[*e_id]),
        }
    }

    pub fn get_source_id(&self, ctx: &SchedEncoder) -> SourceId {
        match self {
            Transition::Cond(_) => self.get_condition(ctx).unwrap().source,
            Transition::Eff(_) => self.get_effect(ctx).unwrap().source,
            Transition::CondEff(_, _) => {
                let res = self.get_condition(ctx).unwrap().source;
                debug_assert!(res == self.get_effect(ctx).unwrap().source);
                res
            }
        }
    }

    pub fn get_prez(&self, ctx: &SchedEncoder) -> Lit {
        //pub fn get_prez(&self, ctx: &mut SchedEncoder) -> Lit {
        match self {
            Transition::Cond(_) => self.get_condition(ctx).unwrap().prez,
            Transition::Eff(_) => self.get_effect(ctx).unwrap().prez,
            Transition::CondEff(_, _) => {
                let res = self.get_condition(ctx).unwrap().prez;
                debug_assert!(res == self.get_effect(ctx).unwrap().prez);
                res
                //ctx.store.reify(aries::model::lang::expr::and([
                //    self.get_condition(ctx).unwrap().prez,
                //    self.get_effect(ctx).unwrap().prez,
                //]))
            }
        }
    }

    pub fn get_state_var<'a>(&self, ctx: &'a SchedEncoder) -> &'a StateVar {
        match self {
            Transition::Cond(_) => &self.get_condition(ctx).unwrap().state_var,
            Transition::Eff(_) => &self.get_effect(ctx).unwrap().state_var,
            Transition::CondEff(_, _) => {
                let res = &self.get_condition(ctx).unwrap().state_var;
                debug_assert!(*res == self.get_effect(ctx).unwrap().state_var);
                res
            }
        }
    }

    pub fn get_terms<'a>(&self, ctx: &'a SchedEncoder) -> TransitionTerms<'a> {
        let args = match self {
            Transition::Cond(_) => &self.get_condition(ctx).unwrap().state_var.args,
            Transition::Eff(_) => &self.get_effect(ctx).unwrap().state_var.args,
            Transition::CondEff(_, _) => &self.get_condition(ctx).unwrap().state_var.args,
        };
        let valfrom = self.get_condition(ctx).map(|c| c.value);
        let valto = self.get_effect(ctx).map(|e| match e.operation {
            crate::EffectOp::Assign(term) => term,
            crate::EffectOp::Step(_) => todo!(),
        });
        debug_assert!(valfrom.is_some() || valto.is_some());

        TransitionTerms(args, valfrom, valto)
    }
}

pub struct TransitionTerms<'a>(&'a [IntTerm], Option<IntTerm>, Option<IntTerm>);

impl<'a> TransitionTerms<'a> {
    pub fn args(&self) -> &[IntTerm] {
        self.0
    }
    pub fn valfrom(&self) -> Option<IntTerm> {
        self.1
    }
    pub fn valto(&self) -> Option<IntTerm> {
        self.2
    }
    pub fn unwrap(&self) -> (&[IntTerm], Option<IntTerm>, Option<IntTerm>) {
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
    store: Vec<Transition>,

    of_condition: DirectIdMap<ConditionId, TransitionId>,
    of_effect: DirectIdMap<EffectId, TransitionId>,
    of_empty_source: Vec<TransitionId>,
    of_concrete_source: DirectIdMap<TaskId, SmallVec<[TransitionId; 6]>>,

    /// For each transition, stores the indices of its (non-constant) terms in its source's arguments
    /// (to avoid constantly recomputing them later).
    pub transition_terms_indices_in_source: Vec<SmallVec<[Option<usize>; 6]>>,
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

    pub fn get(&self, transition_id: TransitionId) -> Option<&Transition> {
        self.store.get(transition_id)
    }
    pub fn get_for_condition(&self, condition_id: ConditionId) -> Option<(TransitionId, &Transition)> {
        self.of_condition
            .get(condition_id)
            .map(|&tr_id| (tr_id, &self.store[tr_id]))
    }
    pub fn get_for_effect(&self, effect_id: EffectId) -> Option<(TransitionId, &Transition)> {
        self.of_effect.get(effect_id).map(|&tr_id| (tr_id, &self.store[tr_id]))
    }
    pub fn get_for_source(&self, source_id: &SourceId) -> Option<impl Iterator<Item = (TransitionId, &Transition)>> {
        match source_id {
            None => Some(self.of_empty_source.iter()),
            Some(task_id) => self.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .map(|iter| iter.map(|&tr_id| (tr_id, &self.store[tr_id])))
    }

    pub fn get_transition_terms_positions_in_source_terms(
        &self,
        transition_id: TransitionId,
    ) -> Option<impl Iterator<Item = &Option<usize>>> {
        self.transition_terms_indices_in_source
            .get(transition_id)
            .map(|v| v.iter())
    }
    pub fn get_transition_term_position_in_source_terms(
        &self,
        transition_id: TransitionId,
        term_index: usize,
    ) -> Option<usize> {
        self.transition_terms_indices_in_source
            .get(transition_id)
            .and_then(|v| v.get(term_index).copied()?)
    }

    pub fn from(
        ctx: &SchedEncoder,
        empty_source_terms: &Vec<IntTerm>,
        concrete_source_terms: &DirectIdMap<TaskId, Vec<IntTerm>>,
    ) -> Self {
        let mut store = vec![];
        let mut of_empty_source = vec![];
        let mut of_concrete_source = DirectIdMap::<TaskId, SmallVec<[TransitionId; 6]>>::default();
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
                    concrete_source_terms.get(src_id.unwrap()).unwrap()
                };
                transition_terms_indices_in_source.push(
                    tr.get_terms(ctx)
                        .into_iter()
                        .map(|term| {
                            if term.is_cst() {
                                None
                            } else {
                                Some(src_terms.iter().position(|&t| t == term).unwrap())
                            }
                        })
                        .collect(),
                );
                if src_id.is_none() {
                    of_empty_source.push(tr_id);
                } else if of_concrete_source.contains_key(src_id.unwrap()) {
                    of_concrete_source.get_mut(src_id.unwrap()).unwrap().push(tr_id);
                } else {
                    of_concrete_source.insert(src_id.unwrap(), smallvec![tr_id]);
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
