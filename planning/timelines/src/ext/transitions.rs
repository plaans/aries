use std::collections::BTreeMap;

use aries::prelude::Lit;
use idmap::DirectIdMap;
use smallvec::{SmallVec, smallvec};

use crate::constraints::HasValueAt;
use crate::encoder::{CondId, SchedEncoder};
use crate::ext::Source;
use crate::{Effect, EffectId, IntTerm, StateVar, TaskId};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Transition {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
    CondEff(CondId, EffectId),
}

impl Transition {
    pub fn get_condition<'a>(&self, ctx: &'a SchedEncoder) -> Option<&'a HasValueAt> {
        match self {
            Transition::Eff(_) => None,
            Transition::Cond(c_id) | Transition::CondEff(c_id, _) => Some(&ctx.causal_links.conditions[*c_id]),
        }
    }
    pub fn get_effect<'a>(&self, ctx: &'a SchedEncoder) -> Option<&'a Effect> {
        match self {
            Transition::Cond(_) => None,
            Transition::Eff(e_id) | Transition::CondEff(_, e_id) => Some(&ctx.sched.effects[*e_id]),
        }
    }
    pub fn get_source(&self, ctx: &SchedEncoder) -> Source {
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
        match self {
            Transition::Cond(_) => self.get_condition(ctx).unwrap().prez,
            Transition::Eff(_) => self.get_effect(ctx).unwrap().prez,
            Transition::CondEff(_, _) => {
                let res = self.get_condition(ctx).unwrap().prez;
                debug_assert!(res == self.get_effect(ctx).unwrap().prez);
                res
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
        let args = &self.get_state_var(ctx).args;
        let valfrom = self.get_condition(ctx).map(|c| c.value);
        let valto = self.get_effect(ctx).map(|e| match e.operation {
            crate::EffectOp::Assign(term) => term,
            crate::EffectOp::Step(_) => todo!(),
        });
        debug_assert!(valfrom.is_some() || valto.is_some());

        TransitionTerms(args, valfrom, valto)
    }

    pub fn with_ctx<'a>(&'a self, ctx: &'a SchedEncoder) -> TransitionWithCtx<'a> {
        TransitionWithCtx(self, ctx)
    }
}

pub struct TransitionWithCtx<'a>(&'a Transition, &'a SchedEncoder);

impl<'a> std::fmt::Debug for TransitionWithCtx<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut _str = String::new();
        write!(
            f,
            "({:?}, {:?}) {:?}: {}",
            self.0,
            self.0.get_source(self.1),
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
    pub fn iter(&self) -> impl Iterator<Item = IntTerm> + use<'a> {
        self.0.iter().copied().chain(self.1).chain(self.2)
    }
}

type TransitionId = usize;

pub struct Transitions {
    store: Vec<Transition>,
    /// For each transition, stores the indices of its (non-constant) terms in
    /// the collection (vector) of all terms appearing in this and and "sibling" transitions,
    /// to prevent from recomputing them too often later.
    transition_terms_indices_in_source: BTreeMap<Transition, SmallVec<[Option<usize>; 6]>>,

    of_condition: DirectIdMap<CondId, TransitionId>,
    of_effect: DirectIdMap<EffectId, TransitionId>,
    of_empty_source: Vec<TransitionId>,
    of_concrete_source: DirectIdMap<TaskId, SmallVec<[TransitionId; 6]>>,
}

impl Transitions {
    pub fn iter(&self) -> impl Iterator<Item = (&Transition, Source)> {
        std::iter::chain(
            self.of_empty_source.iter().map(|tr_id| (&self.store[*tr_id], None)),
            self.of_concrete_source.iter().flat_map(move |(task_id, tr_ids)| {
                tr_ids.iter().map(move |tr_id| (&self.store[*tr_id], Some(task_id)))
            }),
        )
    }

    pub fn get_for_condition(&self, condition_id: CondId) -> Option<&Transition> {
        self.of_condition.get(condition_id).map(|&tr_id| &self.store[tr_id])
    }
    pub fn get_for_effect(&self, effect_id: EffectId) -> Option<&Transition> {
        self.of_effect.get(effect_id).map(|&tr_id| &self.store[tr_id])
    }
    pub fn get_for_source(&self, source: &Source) -> impl Iterator<Item = &Transition> {
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
        transition: &Transition,
    ) -> Option<impl Iterator<Item = Option<usize>>> {
        self.transition_terms_indices_in_source
            .get(transition)
            .map(|v| v.iter().copied())
    }

    pub fn from(
        ctx: &SchedEncoder,
        empty_source_transitions_terms: &Vec<IntTerm>,
        concrete_source_transitions_terms: &DirectIdMap<TaskId, Vec<IntTerm>>,
    ) -> Self {
        let mut store = vec![];
        let mut of_empty_source = vec![];
        let mut of_concrete_source = DirectIdMap::<TaskId, SmallVec<[TransitionId; 6]>>::default();
        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut transition_terms_indices_in_source = BTreeMap::<Transition, SmallVec<[Option<usize>; 6]>>::default();

        let mut add_transition =
            |tr: Transition,
             src: &Source,
             _of_condition: &mut DirectIdMap<CondId, TransitionId>,
             _of_effect: &mut DirectIdMap<EffectId, TransitionId>| {
                let tr_id = store.len();

                let src_terms = if src.is_none() {
                    empty_source_transitions_terms
                } else {
                    concrete_source_transitions_terms.get(src.unwrap()).unwrap()
                };
                transition_terms_indices_in_source.insert(
                    tr,
                    SmallVec::from_iter(tr.get_terms(ctx).iter().map(|term| {
                        if term.is_cst() {
                            None
                        } else {
                            Some(src_terms.iter().position(|&t| t == term).unwrap())
                        }
                    })),
                );
                if src.is_none() {
                    of_empty_source.push(tr_id);
                } else if of_concrete_source.contains_key(src.unwrap()) {
                    of_concrete_source.get_mut(src.unwrap()).unwrap().push(tr_id);
                } else {
                    of_concrete_source.insert(src.unwrap(), smallvec![tr_id]);
                }
                match tr {
                    Transition::Cond(cond_id) => _of_condition.insert(cond_id, tr_id),
                    Transition::Eff(eff_id) => _of_effect.insert(eff_id, tr_id),
                    Transition::CondEff(cond_id, eff_id) => {
                        _of_condition.insert(cond_id, tr_id);
                        _of_effect.insert(eff_id, tr_id)
                    }
                };
                store.push(tr);
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
                                Transition::CondEff(*cond_id, *eff_id),
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
            for (cond_id, _) in cs {
                if !of_condition.contains_key(cond_id) {
                    add_transition(Transition::Cond(*cond_id), src, &mut of_condition, &mut of_effect);
                }
            }
        }
        // Cast all effects that haven't been found to be part of a CondEff as simple Eff transitions
        for (src, es) in &effs_by_source {
            for (eff_id, _) in es {
                if !of_effect.contains_key(eff_id) {
                    add_transition(Transition::Eff(*eff_id), src, &mut of_condition, &mut of_effect);
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
