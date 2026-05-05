use std::collections::BTreeMap;

use aries::prelude::Lit;
use idmap::DirectIdMap;
use smallvec::{SmallVec, smallvec};

use crate::constraints::HasValueAt;
use crate::encoder::{CondId, SchedEncoder};
use crate::ext::{SchedEncoderExt, Source};
use crate::{Effect, EffectId, IntTerm, StateVar, TaskId};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionId {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
    CondEff(CondId, EffectId),
}

impl TransitionId {
    pub fn as_ref<'a>(self, ctx: &'a SchedEncoderExt) -> TransitionRef<'a> {
        match self {
            TransitionId::Cond(c_id) => TransitionRef::Cond(ctx.get_condition(c_id)),
            TransitionId::Eff(e_id) => TransitionRef::Eff(ctx.get_effect(e_id)),
            TransitionId::CondEff(c_id, e_id) => TransitionRef::CondEff(ctx.get_condition(c_id), ctx.get_effect(e_id)),
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum TransitionRef<'a> {
    Cond(&'a HasValueAt),
    Eff(&'a Effect),
    CondEff(&'a HasValueAt, &'a Effect),
}
impl<'a> TransitionRef<'a> {
    pub fn get_condition(&self) -> Option<&'a HasValueAt> {
        match self {
            TransitionRef::Eff(_) => None,
            TransitionRef::Cond(c) | TransitionRef::CondEff(c, _) => Some(c),
        }
    }
    pub fn get_effect(&self) -> Option<&'a Effect> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(e) | TransitionRef::CondEff(_, e) => Some(e),
        }
    }
    pub fn get_source(&self) -> Source {
        match self {
            TransitionRef::Cond(c) => c.source,
            TransitionRef::Eff(e) => e.source,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.source == e.source);
                c.source
            }
        }
    }
    pub fn get_prez(&self) -> Lit {
        match self {
            TransitionRef::Cond(c) => c.prez,
            TransitionRef::Eff(e) => e.prez,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.prez == e.prez);
                c.prez
            }
        }
    }
    pub fn get_state_var(&self) -> &'a StateVar {
        match self {
            TransitionRef::Cond(c) => &c.state_var,
            TransitionRef::Eff(e) => &e.state_var,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.state_var == e.state_var);
                &c.state_var
            }
        }
    }
    pub fn get_args(&self) -> &'a [IntTerm] {
        &self.get_state_var().args
    }
    pub fn get_valfrom(&self) -> Option<&IntTerm> {
        self.get_condition().map(|c| &c.value)
    }
    pub fn get_valto(&self) -> Option<&IntTerm> {
        self.get_effect().map(|e| match &e.operation {
            crate::EffectOp::Assign(term) => term,
            crate::EffectOp::Step(_) => todo!(),
        })
    }
    pub fn get_terms(&self) -> (&'a [IntTerm], Option<&IntTerm>, Option<&IntTerm>) {
        debug_assert!(self.get_valfrom().is_some() || self.get_valto().is_some());
        (self.get_args(), self.get_valfrom(), self.get_valto())
    }
    pub fn terms_len(&self) -> usize {
        self.get_args().len() + self.get_valfrom().is_some() as usize + self.get_valto().is_some() as usize
    }
    pub fn get_term(&self, i: usize) -> &IntTerm {
        if i == self.get_args().len() + 1 {
            self.get_valto().unwrap()
        } else if i == self.get_args().len() {
            self.get_valfrom().unwrap()
        } else {
            &self.get_args()[i]
        }
    }
    pub fn get_valfrom_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => Some(self.get_args().len()),
            TransitionRef::Eff(_) => None,
            TransitionRef::CondEff(_, _) => Some(self.get_args().len()),
        }
    }
    pub fn get_valto_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(_) => Some(self.get_args().len()),
            TransitionRef::CondEff(_, _) => Some(self.get_args().len() + 1),
        }
    }
    /*pub fn terms_iter(&self) -> impl Iterator<Item = &IntTerm> {
        self.args().iter().chain(self.valfrom()).chain(self.valto())
    }*/
    pub fn iter_terms(&self) -> impl Iterator<Item = IntTerm> + use<'_> {
        self.get_args()
            .iter()
            .copied()
            .chain(self.get_valfrom().copied())
            .chain(self.get_valto().copied())
    }
}

impl<'a> std::fmt::Debug for TransitionRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}]({:?}: {:?}->{:?})",
            self.get_source(),
            self.get_state_var(),
            self.get_valfrom(),
            self.get_valto(),
        )
    }
}

type TransitionIndex = usize;

pub struct Transitions {
    store: Vec<TransitionId>,
    /// For each transition, stores the indices of its (non-constant) terms in
    /// the collection (vector) of all terms appearing in this and and "sibling" transitions,
    /// to prevent from recomputing them too often later.
    transition_terms_indices_in_source: BTreeMap<TransitionId, SmallVec<[Option<usize>; 6]>>,

    of_condition: DirectIdMap<CondId, TransitionIndex>,
    of_effect: DirectIdMap<EffectId, TransitionIndex>,
    of_empty_source: Vec<TransitionIndex>,
    of_concrete_source: DirectIdMap<TaskId, SmallVec<[TransitionIndex; 6]>>,
}

impl Transitions {
    pub fn iter(&self) -> impl Iterator<Item = (TransitionId, Source)> {
        std::iter::chain(
            self.of_empty_source.iter().map(|tr_id| (self.store[*tr_id], None)),
            self.of_concrete_source
                .iter()
                .flat_map(move |(task_id, tr_ids)| tr_ids.iter().map(move |tr_id| (self.store[*tr_id], Some(task_id)))),
        )
    }

    pub fn get_for_condition(&self, condition_id: CondId) -> Option<&TransitionId> {
        self.of_condition.get(condition_id).map(|&tr_id| &self.store[tr_id])
    }
    pub fn get_for_effect(&self, effect_id: EffectId) -> Option<&TransitionId> {
        self.of_effect.get(effect_id).map(|&tr_id| &self.store[tr_id])
    }
    pub fn get_for_source(&self, source: &Source) -> impl Iterator<Item = &TransitionId> {
        match source {
            None => Some(self.of_empty_source.iter()),
            Some(task_id) => self.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .unwrap_or([].iter())
        .map(|&tr_id| &self.store[tr_id])
    }

    /// Returns the position of the transition's terms within the source's terms
    /// (i.e. the collection of all terms appearing in the source's transitions).
    /// None corresponds to a constant term.
    pub fn get_transition_terms_positions_in_source_terms(
        &self,
        transition_id: &TransitionId,
    ) -> Option<impl Iterator<Item = Option<usize>>> {
        self.transition_terms_indices_in_source
            .get(transition_id)
            .map(|v| v.iter().copied())
    }

    pub fn from(
        ctx: &SchedEncoder,
        empty_source_transitions_terms: &Vec<IntTerm>,
        concrete_source_transitions_terms: &DirectIdMap<TaskId, Vec<IntTerm>>,
        default_initial_effects: &[Effect],
    ) -> Self {
        let mut store = vec![];
        let mut of_empty_source = vec![];
        let mut of_concrete_source = DirectIdMap::<TaskId, SmallVec<[TransitionIndex; 6]>>::default();
        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut transition_terms_indices_in_source = BTreeMap::<TransitionId, SmallVec<[Option<usize>; 6]>>::default();

        let mut add_transition =
            |tr: (TransitionId, TransitionRef),
             src: &Source,
             _of_condition: &mut DirectIdMap<CondId, TransitionIndex>,
             _of_effect: &mut DirectIdMap<EffectId, TransitionIndex>| {
                let index = store.len();

                let src_terms = if src.is_none() {
                    empty_source_transitions_terms
                } else {
                    concrete_source_transitions_terms.get(src.unwrap()).unwrap()
                };
                transition_terms_indices_in_source.insert(
                    tr.0,
                    SmallVec::from_iter(tr.1.iter_terms().map(|term| {
                        if term.is_cst() {
                            None
                        } else {
                            Some(src_terms.iter().position(|&t| t == term).unwrap())
                        }
                    })),
                );
                if src.is_none() {
                    of_empty_source.push(index);
                } else if of_concrete_source.contains_key(src.unwrap()) {
                    of_concrete_source.get_mut(src.unwrap()).unwrap().push(index);
                } else {
                    of_concrete_source.insert(src.unwrap(), smallvec![index]);
                }
                match tr.0 {
                    TransitionId::Cond(cond_id) => _of_condition.insert(cond_id, index),
                    TransitionId::Eff(eff_id) => _of_effect.insert(eff_id, index),
                    TransitionId::CondEff(cond_id, eff_id) => {
                        _of_condition.insert(cond_id, index);
                        _of_effect.insert(eff_id, index)
                    }
                };
                store.push(tr.0);
            };

        let conds_by_source = {
            let mut res = BTreeMap::<Source, Vec<(CondId, &HasValueAt)>>::new();
            for (cond_id, c) in ctx.causal_links.conditions.iter().enumerate() {
                res.entry(c.source).or_default().push((cond_id, c));
            }
            res
        };
        let effs_by_source = {
            let mut res = BTreeMap::<Source, Vec<(EffectId, &Effect)>>::new();
            for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
                res.entry(e.source).or_default().push((eff_id, e));
            }
            res
        };

        // Search for CondEff transition patterns in each source
        // (except the empty one, in which this pattern is ignored).
        for (src, cs) in &conds_by_source {
            if src.is_some() && effs_by_source.contains_key(src) {
                for (eff_id, e) in effs_by_source.get(src).unwrap() {
                    for (cond_id, c) in cs {
                        debug_assert!(e.source == c.source);
                        if e.state_var == c.state_var && e.prez == c.prez {
                            add_transition(
                                (TransitionId::CondEff(*cond_id, *eff_id), TransitionRef::CondEff(c, e)),
                                src,
                                &mut of_condition,
                                &mut of_effect,
                            );
                        }
                    }
                }
            }
            // After all CondEff transitions in the currently considered source have
            // been found, cast the remaining conditions as simple Cond transitions.
            for (cond_id, c) in cs {
                if !of_condition.contains_key(cond_id) {
                    add_transition(
                        (TransitionId::Cond(*cond_id), TransitionRef::Cond(c)),
                        src,
                        &mut of_condition,
                        &mut of_effect,
                    );
                }
            }
        }
        // Cast all effects that haven't been found to be part of a CondEff as simple Eff transitions
        for (src, es) in &effs_by_source {
            for (eff_id, e) in es {
                if !of_effect.contains_key(eff_id) {
                    add_transition(
                        (TransitionId::Eff(*eff_id), TransitionRef::Eff(e)),
                        src,
                        &mut of_condition,
                        &mut of_effect,
                    );
                }
            }
        }
        // Add default initial effects. (they are ground.)
        let n_eff = of_effect.len();
        for (i, e) in default_initial_effects.iter().enumerate() {
            debug_assert!(e.source.is_none());
            add_transition(
                (TransitionId::Eff(n_eff + i), TransitionRef::Eff(e)),
                &e.source,
                &mut of_condition,
                &mut of_effect,
            );
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
