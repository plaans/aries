use aries::core::views::Term;
use aries::prelude::{DomainsExt, IntCst};
pub use aries::utils::StreamingIterator;

use itertools::Itertools;

use crate::IntTerm;
use crate::ext::{SchedEncoderExt, SourceId, TransitionId, TransitionTerms};

fn get_term_dim(term: &IntTerm, ctx: &SchedEncoderExt) -> usize {
    let (lb, ub) = ctx.main.bounds(term.variable());
    usize::try_from(ub - lb + 1).unwrap()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TermGroundingId(pub usize);

#[derive(Debug)]
pub struct TermGrounding {
    pub term: IntTerm,
    pub assignment: IntCst,
    pub assignment_id: TermGroundingId,
}

pub type TransitionGroundingIdVec = Vec<usize>;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitionGroundingIdFlat(pub usize);

#[derive(Debug)]
pub struct TransitionGrounding {
    pub transition_id: TransitionId,
    pub assignment: Vec<IntCst>,
    pub assignment_idvec: TransitionGroundingIdVec,
    pub dims: Vec<usize>,
}

impl TransitionGrounding {
    pub fn idflat(&self) -> TransitionGroundingIdFlat {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.assignment_idvec.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        TransitionGroundingIdFlat(res)
    }

    pub fn default(transition_id: TransitionId, ctx: &SchedEncoderExt) -> Option<Self> {
        let n = ctx.transitions.get(transition_id)?.get_terms(&ctx.main).len();
        Self::with_idvec(transition_id, vec![0; n], ctx)
    }

    pub fn with_idvec(
        transition_id: TransitionId,
        idvec: TransitionGroundingIdVec,
        ctx: &SchedEncoderExt,
    ) -> Option<Self> {
        let terms = ctx.transitions.get(transition_id)?.get_terms(&ctx.main);
        assert!(idvec.len() == terms.len());

        let dims = terms.iter().map(|term| get_term_dim(term, ctx)).collect_vec();
        debug_assert!(idvec.iter().enumerate().all(|(i, &id)| id < dims[i]));

        Some(Self {
            transition_id,
            assignment: ctx.get_terms_values_from_idvec(terms.iter().zip(idvec.iter().copied())),
            assignment_idvec: idvec,
            dims,
        })
    }

    pub fn to_term_groundings(&self, ctx: &SchedEncoderExt) -> Vec<TermGrounding> {
        ctx.transitions
            .get(self.transition_id)
            .unwrap()
            .get_terms(&ctx.main)
            .into_iter()
            .enumerate()
            .map(|(i, term)| TermGrounding {
                term,
                assignment: self.assignment[i],
                assignment_id: TermGroundingId(self.assignment_idvec[i]),
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
    pub assignment_idvec: SourceGroundingIdVec,
    pub dims: Vec<usize>,
}

impl SourceGrounding {
    pub fn idflat(&self) -> SourceGroundingIdFlat {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.assignment_idvec.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        SourceGroundingIdFlat(res)
    }

    pub fn default(source_id: SourceId, ctx: &SchedEncoderExt) -> Self {
        Self::with_idvec(source_id, vec![0; ctx.get_source_terms(source_id).len()], ctx)
    }

    pub fn with_idvec(source_id: SourceId, idvec: SourceGroundingIdVec, ctx: &SchedEncoderExt) -> Self {
        let terms = ctx.get_source_terms(source_id);
        assert!(idvec.len() == terms.len());

        let dims = terms.iter().map(|term| get_term_dim(term, ctx)).collect_vec();
        debug_assert!(idvec.iter().enumerate().all(|(i, &id)| id < dims[i]));

        Self {
            source_id,
            assignment: ctx.get_terms_values_from_idvec(terms.iter().zip(idvec.iter().copied())),
            assignment_idvec: idvec,
            dims,
        }
    }

    pub fn to_term_groundings(&self, ctx: &SchedEncoderExt) -> Vec<TermGrounding> {
        ctx.get_source_terms(self.source_id)
            .iter()
            .enumerate()
            .map(|(i, &term)| TermGrounding {
                term,
                assignment: self.assignment[i],
                assignment_id: TermGroundingId(self.assignment_idvec[i]),
            })
            .collect_vec()
    }

    pub fn get_transitions_groundings(
        &self,
        ctx: &SchedEncoderExt,
    ) -> Option<impl Iterator<Item = TransitionGrounding>> {
        ctx.transitions
            .get_for_source(&self.source_id)
            .map(|tr_ids| tr_ids.map(|(tr_id, _)| self.get_transition_grounding(tr_id, ctx).unwrap()))
    }

    pub fn get_transition_grounding(
        &self,
        transition_id: TransitionId,
        ctx: &SchedEncoderExt,
    ) -> Option<TransitionGrounding> {
        if self.source_id != ctx.transitions.get(transition_id)?.get_source_id(&ctx.main) {
            return None;
        }
        TransitionGrounding::with_idvec(
            transition_id,
            ctx.transitions
                .get_transition_terms_positions_in_source_terms(transition_id)?
                .map(|i| match i {
                    Some(i) => self.assignment_idvec[*i],
                    None => 0,
                })
                .collect(),
            ctx,
        )
    }

    pub fn contains(&self, transition_grounding: &TransitionGrounding, ctx: &SchedEncoderExt) -> Option<bool> {
        assert!(
            self.source_id
                == ctx
                    .transitions
                    .get(transition_grounding.transition_id)?
                    .get_source_id(&ctx.main)
        );

        Some(
            ctx.transitions
                .get_transition_terms_positions_in_source_terms(transition_grounding.transition_id)?
                .enumerate()
                .filter_map(|(j, &i)| i.map(|i| (j, i)))
                .all(|(j, i)| self.assignment[j] == transition_grounding.assignment[i]),
        )
    }

    pub fn absurd(&self, _ctx: &SchedEncoderExt) -> bool {
        // todo!() // TODO
        false // FIXME
    }
}

pub struct TransitionGroundingsIter<'a> {
    ctx: &'a SchedEncoderExt,
    ctx_terms: TransitionTerms<'a>,
    fixed_ids: Vec<Option<usize>>,

    current: Option<TransitionGrounding>,
    is_started: bool,
}

impl<'a> TransitionGroundingsIter<'a> {
    pub fn new(transition_id: TransitionId, ctx: &'a SchedEncoderExt) -> Option<Self> {
        Self::with_fixed(transition_id, &[], ctx)
    }

    pub fn with_fixed(
        transition_id: TransitionId,
        fixed_ids: &[(usize, usize)],
        ctx: &'a SchedEncoderExt,
    ) -> Option<Self> {
        let ctx_terms = ctx.transitions.get(transition_id)?.get_terms(&ctx.main);
        assert!(fixed_ids.iter().map(|(i, _)| i).all_unique());

        let (id, fixed_ids) = {
            let mut res1 = vec![0; ctx_terms.len()];
            let mut res2 = vec![None; ctx_terms.len()];
            for &(i, id) in fixed_ids {
                res1[i] = id;
                res2[i] = Some(id)
            }
            (res1, res2)
        };

        let current = Some(TransitionGrounding::with_idvec(transition_id, id, ctx)?);

        Some(Self {
            ctx,
            ctx_terms,
            fixed_ids,
            current,
            is_started: false,
        })
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
        if current.assignment_idvec.is_empty() {
            self.current = None;
            return;
        }
        let mut i = current.assignment_idvec.len() - 1;
        loop {
            if current.assignment_idvec[i] == current.dims[i] - 1 {
                if i == 0 {
                    self.current = None;
                    return;
                }
                if self.fixed_ids[i].is_none() {
                    current.assignment_idvec[i] = 0;
                    current.assignment[i] = self
                        .ctx
                        .get_term_value_from_id(self.ctx_terms.get(i), current.assignment_idvec[i])
                        .unwrap();
                }
                i -= 1;
            } else {
                if self.fixed_ids[i].is_some() {
                    continue;
                }
                current.assignment_idvec[i] += 1;
                current.assignment[i] = self
                    .ctx
                    .get_term_value_from_id(self.ctx_terms.get(i), current.assignment_idvec[i])
                    .unwrap();
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}

pub struct SourceGroundingsIter<'a> {
    ctx: &'a SchedEncoderExt,
    ctx_terms: &'a [IntTerm],
    fixed_ids: Vec<Option<usize>>,

    current: Option<SourceGrounding>,
    is_started: bool,
}

impl<'a> SourceGroundingsIter<'a> {
    pub fn new(source_id: SourceId, ctx: &'a SchedEncoderExt) -> Self {
        Self::with_fixed(source_id, &[], ctx)
    }

    pub fn with_fixed(source_id: SourceId, fixed_ids: &[(usize, usize)], ctx: &'a SchedEncoderExt) -> Self {
        let ctx_terms = ctx.get_source_terms(source_id);
        assert!(fixed_ids.iter().map(|(i, _)| i).all_unique());

        let (id, fixed_ids) = {
            let mut res1 = vec![0; ctx_terms.len()];
            let mut res2 = vec![None; ctx_terms.len()];
            for &(i, id) in fixed_ids {
                res1[i] = id;
                res2[i] = Some(id)
            }
            (res1, res2)
        };

        let current = Some(SourceGrounding::with_idvec(source_id, id, ctx));

        Self {
            ctx,
            ctx_terms,
            fixed_ids,
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
        if current.assignment_idvec.is_empty() {
            self.current = None;
            return;
        }
        let mut i = current.assignment_idvec.len() - 1;
        loop {
            if current.assignment_idvec[i] == current.dims[i] - 1 {
                if i == 0 {
                    self.current = None;
                    return;
                }
                if self.fixed_ids[i].is_none() {
                    current.assignment_idvec[i] = 0;
                    current.assignment[i] = self
                        .ctx
                        .get_term_value_from_id(&self.ctx_terms[i], current.assignment_idvec[i])
                        .unwrap();
                }
                i -= 1;
            } else {
                if self.fixed_ids[i].is_some() {
                    continue;
                }
                current.assignment_idvec[i] += 1;
                current.assignment[i] = self
                    .ctx
                    .get_term_value_from_id(&self.ctx_terms[i], current.assignment_idvec[i])
                    .unwrap();
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}
