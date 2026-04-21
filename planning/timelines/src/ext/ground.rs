use aries::core::views::Term;
use aries::prelude::{DomainsExt, IntCst};
use aries::utils::StreamingIterator;

use itertools::Itertools;

use crate::IntTerm;
use crate::encoder::SchedEncoder;
use crate::ext::{
    SourceId, TransitionId, TransitionTerms, get_source_terms, term_value_from_id, terms_values_from_ids,
};

pub fn iter_transition_groundings<'a>(
    transition_id: TransitionId,
    ctx: &'a SchedEncoder,
) -> impl StreamingIterator<Item = TransitionGrounding> {
    TransitionGroundingsIter::new(transition_id, ctx)
}

pub fn iter_source_groundings<'a>(
    source_id: SourceId,
    ctx: &'a SchedEncoder,
) -> impl StreamingIterator<Item = SourceGrounding> {
    SourceGroundingsIter::new(source_id, ctx).filter(|src_gr| !src_gr.is_absurd(ctx))
}

pub fn iter_source_groundings_containing_transition_grounding<'a>(
    transition_grounding: &'a TransitionGrounding,
    ctx: &'a SchedEncoder,
) -> impl StreamingIterator<Item = SourceGrounding> {
    let source_id = ctx.ext.as_ref().unwrap().transitions.store[transition_grounding.transition_id].get_source(ctx);
    iter_source_groundings(source_id, ctx).filter(move |src_gr| src_gr.contains(transition_grounding, ctx))
}

fn get_term_dim(term: &IntTerm, ctx: &SchedEncoder) -> usize {
    let (lb, ub) = ctx.bounds(term.variable());
    usize::try_from(ub - lb + 1).unwrap()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TermGroundingId(pub usize);

#[derive(Debug)]
pub struct TermGrounding {
    pub term: IntTerm,
    pub assignment: IntCst,
    pub id: TermGroundingId,
}

pub type TransitionGroundingIdVec = Vec<usize>;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitionGroundingIdFlat(pub usize);

#[derive(Debug)]
pub struct TransitionGrounding {
    pub transition_id: TransitionId,
    pub assignment: Vec<IntCst>,
    pub id: TransitionGroundingIdVec,
    pub dims: Vec<usize>,
}

impl TransitionGrounding {
    pub fn id_flat(&self) -> TransitionGroundingIdFlat {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.id.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        TransitionGroundingIdFlat(res)
    }

    pub fn default(transition_id: TransitionId, ctx: &SchedEncoder) -> Self {
        Self::with_id(
            transition_id,
            vec![
                0;
                ctx.ext.as_ref().unwrap().transitions.store[transition_id]
                    .get_terms(ctx)
                    .len()
            ],
            ctx,
        )
    }

    pub fn with_id(transition_id: TransitionId, id: TransitionGroundingIdVec, ctx: &SchedEncoder) -> Self {
        let terms = ctx.ext.as_ref().unwrap().transitions.store[transition_id].get_terms(ctx);
        assert!(id.len() == terms.len());

        let dims = terms.iter().map(|term| get_term_dim(term, ctx)).collect_vec();
        debug_assert!(id.iter().enumerate().all(|(i, &id)| id < dims[i]));

        Self {
            transition_id,
            assignment: terms_values_from_ids(terms.iter(), &id, ctx),
            id,
            dims,
        }
    }

    pub fn to_term_groundings(&self, ctx: &SchedEncoder) -> Vec<TermGrounding> {
        ctx.ext.as_ref().unwrap().transitions.store[self.transition_id]
            .get_terms(ctx)
            .into_iter()
            .enumerate()
            .map(|(i, term)| TermGrounding {
                term,
                assignment: self.assignment[i],
                id: TermGroundingId(self.id[i]),
            })
            .collect_vec()
    }
}

pub type SourceGroundingIdVec = Vec<usize>;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceGroundingIdFlat(pub usize);

pub struct SourceGrounding {
    pub source_id: SourceId,
    pub assignment: Vec<IntCst>,
    pub id: SourceGroundingIdVec,
    pub dims: Vec<usize>,
}

impl SourceGrounding {
    pub fn id_flat(&self) -> SourceGroundingIdFlat {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.id.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        SourceGroundingIdFlat(res)
    }

    pub fn default(source_id: SourceId, ctx: &SchedEncoder) -> Self {
        Self::with_id(source_id, vec![0; get_source_terms(source_id, ctx).len()], ctx)
    }

    pub fn with_id(source_id: SourceId, id: SourceGroundingIdVec, ctx: &SchedEncoder) -> Self {
        let terms = get_source_terms(source_id, ctx);
        assert!(id.len() == terms.len());

        let dims = terms.iter().map(|term| get_term_dim(term, ctx)).collect_vec();
        debug_assert!(id.iter().enumerate().all(|(i, &id)| id < dims[i]));

        Self {
            source_id,
            assignment: terms_values_from_ids(terms.iter(), &id, ctx),
            id,
            dims,
        }
    }

    pub fn to_term_groundings(&self, ctx: &SchedEncoder) -> Vec<TermGrounding> {
        get_source_terms(self.source_id, ctx)
            .iter()
            .enumerate()
            .map(|(i, &term)| TermGrounding {
                term,
                assignment: self.assignment[i],
                id: TermGroundingId(self.id[i]),
            })
            .collect_vec()
    }

    pub fn get_transitions_groundings(
        &self,
        ctx: &SchedEncoder,
    ) -> Option<impl Iterator<Item = TransitionGrounding>> {
        ctx.ext
            .as_ref()
            .unwrap()
            .transitions
            .of_source(&self.source_id)
            .map(|tr_ids| {
                tr_ids
                    .iter()
                    .map(|&tr_id| self.get_transition_grounding(tr_id, ctx).unwrap())
            })
    }

    pub fn get_transition_grounding(
        &self,
        transition_id: TransitionId,
        ctx: &SchedEncoder,
    ) -> Option<TransitionGrounding> {
        if self.source_id != ctx.ext.as_ref().unwrap().transitions.store[transition_id].get_source(ctx) {
            return None;
        }
        Some(TransitionGrounding::with_id(
            transition_id,
            ctx.ext.as_ref().unwrap().transitions.transition_terms_indices_in_source[transition_id]
                .iter()
                .map(|&i| self.id[i])
                .collect_vec(),
            ctx,
        ))
    }

    pub fn contains(&self, transition_grounding: &TransitionGrounding, ctx: &SchedEncoder) -> bool {
        assert!(
            self.source_id
                == ctx.ext.as_ref().unwrap().transitions.store[transition_grounding.transition_id].get_source(ctx)
        );

        ctx.ext.as_ref().unwrap().transitions.transition_terms_indices_in_source[transition_grounding.transition_id]
            .iter()
            .enumerate()
            .all(|(j, &i)| self.id[j] == transition_grounding.id[i])
    }

    pub fn is_absurd(&self, _ctx: &SchedEncoder) -> bool {
        // todo!() // TODO
        false // FIXME
    }
}

struct TransitionGroundingsIter<'a> {
    ctx: &'a SchedEncoder,
    ctx_terms: TransitionTerms<'a>,
    fixed_indices: Vec<Option<usize>>,

    current: Option<TransitionGrounding>,
    is_started: bool,
}

impl<'a> TransitionGroundingsIter<'a> {
    fn new(transition_id: TransitionId, ctx: &'a SchedEncoder) -> Self {
        Self::with_fixed(transition_id, &[], ctx)
    }

    fn with_fixed(transition_id: TransitionId, fixed_indices: &[(usize, usize)], ctx: &'a SchedEncoder) -> Self {
        let ctx_terms = ctx.ext.as_ref().unwrap().transitions.store[transition_id].get_terms(ctx);
        assert!(fixed_indices.iter().map(|(i, _)| i).all_unique());

        let (id, fixed_indices) = {
            let mut res1 = vec![0; ctx_terms.len()];
            let mut res2 = vec![None; ctx_terms.len()];
            for &(i, id) in fixed_indices {
                res1[i] = id;
                res2[i] = Some(id)
            }
            (res1, res2)
        };

        let current = Some(TransitionGrounding::with_id(transition_id, id, ctx));

        Self {
            ctx,
            ctx_terms,
            fixed_indices,
            current,
            is_started: false,
        }
    }
}

impl<'a> StreamingIterator for TransitionGroundingsIter<'a> {
    type Item = TransitionGrounding;

    fn advance(&mut self) {
        if !self.is_started {
            self.is_started = true;
            return;
        }
        let Some(current) = &mut self.current else { return };
        if current.id.is_empty() {
            self.current = None;
            return;
        }
        let mut i = current.id.len() - 1;
        loop {
            if current.id[i] == current.dims[i] - 1 {
                if i == 0 {
                    self.current = None;
                    return;
                }
                if self.fixed_indices[i].is_none() {
                    current.id[i] = 0;
                    current.assignment[i] = term_value_from_id(self.ctx_terms.get(i), current.id[i], self.ctx).unwrap();
                }
                i -= 1;
            } else {
                if self.fixed_indices[i].is_some() {
                    continue;
                }
                current.id[i] += 1;
                current.assignment[i] = term_value_from_id(self.ctx_terms.get(i), current.id[i], self.ctx).unwrap();
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}

struct SourceGroundingsIter<'a> {
    ctx: &'a SchedEncoder,
    ctx_terms: &'a [IntTerm],
    fixed_indices: Vec<Option<usize>>,

    current: Option<SourceGrounding>,
    is_started: bool,
}

impl<'a> SourceGroundingsIter<'a> {
    fn new(source_id: SourceId, ctx: &'a SchedEncoder) -> Self {
        Self::with_fixed(source_id, &[], ctx)
    }

    fn with_fixed(source_id: SourceId, fixed_indices: &[(usize, usize)], ctx: &'a SchedEncoder) -> Self {
        let ctx_terms = get_source_terms(source_id, ctx);
        assert!(fixed_indices.iter().map(|(i, _)| i).all_unique());

        let (id, fixed_indices) = {
            let mut res1 = vec![0; ctx_terms.len()];
            let mut res2 = vec![None; ctx_terms.len()];
            for &(i, id) in fixed_indices {
                res1[i] = id;
                res2[i] = Some(id)
            }
            (res1, res2)
        };

        let current = Some(SourceGrounding::with_id(source_id, id, ctx));

        Self {
            ctx,
            ctx_terms,
            fixed_indices,
            current,
            is_started: false,
        }
    }
}

impl<'a> StreamingIterator for SourceGroundingsIter<'a> {
    type Item = SourceGrounding;

    fn advance(&mut self) {
        if !self.is_started {
            self.is_started = true;
            return;
        }
        let Some(current) = &mut self.current else { return };
        if current.id.is_empty() {
            self.current = None;
            return;
        }
        let mut i = current.id.len() - 1;
        loop {
            if current.id[i] == current.dims[i] - 1 {
                if i == 0 {
                    self.current = None;
                    return;
                }
                if self.fixed_indices[i].is_none() {
                    current.id[i] = 0;
                    current.assignment[i] = term_value_from_id(&self.ctx_terms[i], current.id[i], self.ctx).unwrap();
                }
                i -= 1;
            } else {
                if self.fixed_indices[i].is_some() {
                    continue;
                }
                current.id[i] += 1;
                current.assignment[i] = term_value_from_id(&self.ctx_terms[i], current.id[i], self.ctx).unwrap();
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}
