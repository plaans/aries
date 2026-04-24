pub mod ground;
pub mod lprelax;
pub mod transitions;

use std::collections::BTreeSet;

use crate::TaskId;
use crate::{IntTerm, encoder::SchedEncoder};

use aries::core::views::{Dom, Term};
use aries::model::lang::ModelWrapper;
use aries::prelude::*;

pub(crate) use ground::*;
use idmap::DirectIdMap;
pub(crate) use transitions::*;

pub struct SchedEncoderExt {
    pub(crate) main: std::sync::Arc<SchedEncoder>,
    
    pub(crate) transitions: Transitions,

    empty_source_terms: Vec<IntTerm>,
    concrete_source_terms: DirectIdMap<TaskId, Vec<IntTerm>>,
    
    pub(crate) lprelax: Option<aries_lprelax::LpRelax>,
}

impl Dom for SchedEncoderExt {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.main.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.main.presence(var)
    }
}
impl ModelWrapper for SchedEncoderExt {
    type Lbl = String;

    fn get_model(&self) -> &crate::Model {
        self.main.get_model()
    }

    fn get_model_mut(&mut self) -> &mut crate::Model {
        std::sync::Arc::get_mut(&mut self.main).unwrap().get_model_mut()
    }
}

impl SchedEncoderExt {
    pub fn new(sched_encoder: std::sync::Arc<SchedEncoder>) -> Self {
        let empty_source_terms = collect_empty_source_terms(&sched_encoder);
        let concrete_source_terms = collect_concrete_source_terms(&sched_encoder);
        Self {
            main: sched_encoder.clone(),
            transitions: Transitions::from(&sched_encoder, &empty_source_terms, &concrete_source_terms),
            empty_source_terms,
            concrete_source_terms,
            lprelax: None,
        }
    }

    pub fn iter_transition_groundings(
        &self,
        transition_id: TransitionId,
    ) -> Option<impl StreamingIterator<Item = TransitionGrounding>> {
        ground::TransitionGroundingsIter::new(transition_id, self)
    }

    pub fn iter_source_groundings(&self, source_id: SourceId) -> impl StreamingIterator<Item = SourceGrounding> {
        ground::SourceGroundingsIter::new(source_id, self).filter(|src_gr| !src_gr.absurd(self))
    }

    pub fn iter_source_groundings_containing_transition_grounding(
        &self,
        transition_grounding: &TransitionGrounding,
    ) -> Option<impl StreamingIterator<Item = SourceGrounding>> {
        let source_id = self
            .transitions
            .get(transition_grounding.transition_id)?
            .get_source_id(&self.main);
        Some(
            self.iter_source_groundings(source_id)
                .filter(move |src_gr| src_gr.contains(transition_grounding, self).unwrap()),
        )
    }

    pub fn get_source_terms(&self, source_id: SourceId) -> &[IntTerm] {
        if let Some(task_id) = source_id {
            &self.concrete_source_terms[task_id]
        } else {
            &self.empty_source_terms
        }
    }
    pub fn get_terms_values_from_idvec<'a>(
        &self,
        terms_idvec: impl Iterator<Item = (&'a IntTerm, usize)>,
    ) -> Vec<IntCst> {
        terms_idvec
            .map(|(term, id)| self.get_term_value_from_id(term, id).unwrap())
            .collect()
    }
    pub fn get_term_value_from_id(&self, term: &IntTerm, id: usize) -> Option<IntCst> {
        let (lb, ub) = self.main.bounds(term.variable());
        (id < usize::try_from(ub - lb + 1).unwrap())
            .then(|| term.cst() + lb + term.factor() * IntCst::try_from(id).unwrap())
    }
fn collect_empty_source_terms(ctx: &SchedEncoder) -> Vec<IntTerm> {
    BTreeSet::from_iter(
        std::iter::chain(
            ctx.sched
                .effects
                .iter()
                .enumerate()
                .filter(|&(_eid, e)| e.source.is_none())
                .flat_map(|(eid, _e)| Transition::Eff(eid).get_terms(ctx).into_iter()),
            ctx.causal_links
                .destinations
                .iter()
                .enumerate()
                .filter(|&(_cid, c)| c.source.is_none())
                .flat_map(|(cid, _c)| Transition::Cond(cid).get_terms(ctx).into_iter()),
        )
        .filter(|term| !term.is_cst()),
    )
    .into_iter()
    .collect()
}

fn collect_concrete_source_terms(ctx: &SchedEncoder) -> DirectIdMap<TaskId, Vec<IntTerm>> {
    let mut res = DirectIdMap::<TaskId, BTreeSet<IntTerm>>::new();

    for (_, e) in ctx.sched.effects.iter().enumerate() {
        if let Some(task_id) = e.source {
            if !res.contains_key(task_id) {
                res.insert(task_id, BTreeSet::new());
            }
            res.get_mut(task_id).unwrap().extend(&e.state_var.args);
        }
    }
    for (_, c) in ctx.causal_links.destinations.iter().enumerate() {
        if let Some(task_id) = c.source {
            if !res.contains_key(task_id) {
                res.insert(task_id, BTreeSet::new());
            }
            res.get_mut(task_id).unwrap().extend(&c.state_var.args);
        }
    }

    let res = DirectIdMap::from_iter(res.into_iter().map(|(task_id, set)| (task_id, Vec::from_iter(set))));
    res
}