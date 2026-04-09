use std::collections::HashMap;

use aries::{prelude::IntCst, utils::StreamingIterator};
use idmap::{DirectIdMap, DirectIdSet};

use crate::{ConstraintID, Effect, EffectId, Effects, Model, TaskId, constraints::HasValueAt};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransitionId {
    /// Value is the index of a condition (constraint) in a reference vector of constraints.
    Cond(ConstraintID),
    /// Value is the index/identified of an effect in a collection of them.
    Eff(EffectId),
    /// Combination of Cond and Eff variants.
    CondEff(ConstraintID, EffectId),
}

pub struct Transitions {
    store: HashMap<TransitionId, Vec<Vec<IntCst>>>,
}
impl Transitions {
    pub fn new_ground(effects: &Effects, conditions: &DirectIdMap<ConstraintID, HasValueAt>, model: &Model) -> Self {
        let mut res = Self::new_lifted(effects, conditions);
        res.populate_groundings(effects, conditions, model, true);
        res
    }

    pub fn new_lifted(effects: &Effects, conditions: &DirectIdMap<ConstraintID, HasValueAt>) -> Self {
        let mut effs: HashMap<Option<TaskId>, Vec<(EffectId, &Effect)>> = HashMap::new();
        let mut conds: HashMap<Option<TaskId>, Vec<(ConstraintID, &HasValueAt)>> = HashMap::new();

        for (id, e) in effects.iter().enumerate() {
            effs.entry(e.source)
                .and_modify(|v| v.push((id, e)))
                .or_insert(vec![(id, e)]);
        }
        for (id, c) in conditions.iter() {
            conds
                .entry(c.source)
                .and_modify(|v| v.push((id, c)))
                .or_insert(vec![(id, c)]);
        }

        let mut store = HashMap::new();
        let mut e_in_condeff = DirectIdSet::new();
        let mut c_in_condeff = DirectIdSet::new();

        for (e_src, es) in effs {
            for &(e_id, e) in &es {
                if let Some(cs) = conds.get(&e_src) {
                    for &(c_id, c) in cs {
                        if e.state_var == c.state_var {
                            store.insert(TransitionId::CondEff(c_id, e_id), vec![]);
                            c_in_condeff.insert(c_id);
                            e_in_condeff.insert(e_id);
                        }
                    }
                }
            }
            for (e_id, _) in es {
                if !e_in_condeff.contains(e_id) {
                    store.insert(TransitionId::Eff(e_id), vec![]);
                }
            }
        }
        for (_, cs) in conds {
            for (c_id, _) in cs {
                if !c_in_condeff.contains(c_id) {
                    store.insert(TransitionId::Cond(c_id), vec![]);
                }
            }
        }
        Self { store }
    }

    pub fn populate_groundings(
        &mut self,
        effects: &Effects,
        conditions: &DirectIdMap<ConstraintID, HasValueAt>,
        model: &Model,
        remove_static: bool,
    ) {
        let mut statics = vec![];

        for (transition_id, groundings) in self.store.iter_mut() {
            groundings.clear();

            let mut gs = match &transition_id {
                TransitionId::Cond(c_id) => {
                    let c = conditions.get(c_id).unwrap();
                    c.value_box(model).as_ref().drop_head(1).points()
                }
                TransitionId::Eff(e_id) => {
                    let e = effects.get(*e_id);
                    e.value_box(model).as_ref().drop_head(1).points()
                }
                TransitionId::CondEff(c_id, _) => {
                    let c = conditions.get(c_id).unwrap();
                    c.value_box(model).as_ref().drop_head(1).points()
                }
            };

            let mut is_static = false;
            while let Some(g) = gs.next() {
                is_static = true;
                groundings.push(g.to_vec());
            }
            if !is_static {
                statics.push(transition_id.clone())
            }
        }
        if remove_static {
            for transition_id in statics {
                self.store.remove(&transition_id);
            }
        }
    }
    pub fn groundings_iter(&self, transition_id: &TransitionId) -> Option<impl Iterator<Item = &[IntCst]>> {
        self.store
            .get(transition_id)
            .map(|groundings| groundings.iter().map(Vec::as_slice))
    }
}
