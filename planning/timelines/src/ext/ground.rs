use aries::core::views::Term;
use aries::core::{INT_CST_MAX, INT_CST_MIN, LongCst};
use aries::prelude::{DomainsExt, IntCst};
pub use aries::utils::StreamingIterator;

use smallvec::SmallVec;

use crate::IntTerm;
use crate::ext::SchedEncoderExt;
use crate::ext::encoder::Source;
use crate::ext::transition::{TransitionId, TransitionRef};

impl<'a> SchedEncoderExt<'a> {
    pub fn iter_transition_groundings(
        &'a self,
        transition_id: TransitionId,
    ) -> impl StreamingIterator<Item = TransitionTermsGround<'a>> {
        TransitionTermsGroundIter::new(transition_id, self)
    }

    pub fn iter_source_groundings(&self, source: Source) -> impl StreamingIterator<Item = SourceTermsGround> {
        SourceTermsGroundIter::new(source, self).filter(|src_gr| !src_gr.absurd(self))
    }

    pub fn iter_source_groundings_containing_transition_grounding(
        &self,
        transition_grounding: &TransitionTermsGround,
    ) -> impl StreamingIterator<Item = SourceTermsGround> {
        let source = transition_grounding.transition_ref.get_source();
        self.iter_source_groundings(source)
            .filter(move |src_gr| src_gr.contains(transition_grounding, self))
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TermGroundId(pub usize);

struct FlattenableGround<Id: From<usize>, const N: usize> {
    pub idvec: SmallVec<[TermGroundId; N]>,
    dims: SmallVec<[usize; N]>,
    pub id: Id,
}
impl<Id: From<usize>, const N: usize> FlattenableGround<Id, N> {
    fn from(idvec: impl Iterator<Item = TermGroundId>, bounds: impl Iterator<Item = (IntCst, IntCst)>) -> Self {
        let idvec = idvec.collect::<SmallVec<_>>();
        let dims = bounds
            .inspect(|&(lb, ub)| {
                debug_assert!(lb > INT_CST_MIN);
                debug_assert!(ub < INT_CST_MAX);
                debug_assert!(((ub as LongCst) - (lb as LongCst) + 1) < (INT_CST_MAX as LongCst));
            })
            .map(|(lb, ub)| usize::try_from(ub - lb + 1).unwrap())
            .collect::<SmallVec<_>>();
        debug_assert!(idvec.len() == dims.len());
        debug_assert!(
            idvec
                .iter()
                .zip(dims.iter())
                .all(|(k, &d): (&TermGroundId, &usize)| k.0 < d)
        );
        let id = {
            let mut res = 0;
            let mut factor = 1;
            for (id, &d) in idvec.iter().zip(dims.iter()).rev() {
                debug_assert!(id.0 < d);
                res += id.0 * factor;
                factor *= d;
            }
            Id::from(res)
        };
        Self { idvec, dims, id }
    }
    fn update_id(&mut self) {
        let mut res = 0;
        let mut factor = 1;
        for (id, &d) in self.idvec.iter().zip(self.dims.iter()).rev() {
            debug_assert!(id.0 < d);
            res += id.0 * factor;
            factor *= d;
        }
        self.id = Id::from(res);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitionTermsGroundId(pub usize);
impl From<usize> for TransitionTermsGroundId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceTermsGroundId(pub usize);
impl From<usize> for SourceTermsGroundId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

pub struct TermGround {
    pub term: IntTerm,
    pub id: TermGroundId,
}
impl TermGround {
    pub fn default(term: IntTerm, ctx: &SchedEncoderExt) -> Self {
        debug_assert!(
            !term.is_cst(),
            "Should not be any case when a constant term is considered."
        );
        Self::from(term, TermGroundId::default(), ctx)
    }
    pub fn from(term: IntTerm, id: TermGroundId, ctx: &SchedEncoderExt) -> Self {
        debug_assert!(
            !term.is_cst(),
            "Should not be any case when a constant term is considered."
        );
        {
            let (lb_inner, ub_inner) = ctx.bounds(term.variable());
            debug_assert!(lb_inner > INT_CST_MIN);
            debug_assert!(ub_inner < INT_CST_MAX);
            debug_assert!(((ub_inner as LongCst) - (lb_inner as LongCst) + 1) < (INT_CST_MAX as LongCst));
            debug_assert!(id.0 < usize::try_from(ub_inner - lb_inner + 1).unwrap());
        }
        Self { term, id }
    }
    pub fn assignment(&self, ctx: &SchedEncoderExt) -> IntCst {
        let (lb_inner, _) = ctx.bounds(self.term.variable());
        let x = IntCst::try_from(self.id.0).unwrap() + lb_inner;
        self.term.cst() + self.term.factor() * x
    }
}

pub struct TransitionTermsGround<'a> {
    pub transition_id: TransitionId,
    pub transition_ref: TransitionRef<'a>,
    flattenable: FlattenableGround<TransitionTermsGroundId, 6>,
    assignment: SmallVec<[IntCst; 6]>,
}
impl<'a> TransitionTermsGround<'a> {
    pub fn get_idvec(&self) -> &[TermGroundId] {
        &self.flattenable.idvec
    }
    pub fn get_id(&self) -> TransitionTermsGroundId {
        self.flattenable.id
    }

    fn update_assignment(&mut self, ctx: &SchedEncoderExt) {
        self.assignment = self
            .to_term_groundings(ctx)
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();
    }
    pub fn get_assignment(&self) -> &[IntCst] {
        &self.assignment
    }
    pub fn to_term_groundings(&self, ctx: &SchedEncoderExt) -> impl Iterator<Item = TermGround> {
        self.transition_ref
            .iter_terms()
            .zip(self.flattenable.idvec.iter())
            .map(|(term, &id)| TermGround::from(term, id, ctx))
    }

    pub fn default(transition_id: TransitionId, ctx: &'a SchedEncoderExt) -> Self {
        let transition_ref = ctx.get_transition(transition_id);
        let n = transition_ref.terms_len();
        Self::from(transition_id, std::iter::repeat_n(TermGroundId::default(), n), ctx)
    }
    pub fn from(
        transition_id: TransitionId,
        idvec: impl Iterator<Item = TermGroundId>,
        ctx: &'a SchedEncoderExt,
    ) -> Self {
        let transition_ref = ctx.get_transition(transition_id);
        let flattenable = FlattenableGround::from(
            idvec,
            transition_ref.iter_terms().map(|term| ctx.bounds(term.variable())),
        );
        let assignment = transition_ref
            .iter_terms()
            .zip(flattenable.idvec.iter())
            .map(|(term, id)| TermGround::from(term, *id, ctx))
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();

        Self {
            transition_id,
            transition_ref,
            flattenable,
            assignment,
        }
    }
}

pub struct SourceTermsGround {
    pub source: Source,
    flattenable: FlattenableGround<SourceTermsGroundId, 6>,
    assignment: SmallVec<[IntCst; 6]>,
}

impl<'a> SourceTermsGround {
    pub fn get_idvec(&self) -> &[TermGroundId] {
        &self.flattenable.idvec
    }
    pub fn get_id(&self) -> SourceTermsGroundId {
        self.flattenable.id
    }

    fn update_assignment(&mut self, ctx: &SchedEncoderExt) {
        self.assignment = self
            .to_term_groundings(ctx)
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();
    }
    pub fn get_assignment(&self) -> &[IntCst] {
        &self.assignment
    }
    pub fn to_term_groundings(&self, ctx: &SchedEncoderExt) -> impl Iterator<Item = TermGround> {
        ctx.get_source_terms(&self.source)
            .iter()
            .zip(self.flattenable.idvec.iter())
            .map(|(&term, id)| TermGround::from(term, *id, ctx))
    }

    pub fn default(source: Source, ctx: &SchedEncoderExt) -> Self {
        let n = ctx.get_source_terms(&source).len();
        Self::from(source, std::iter::repeat_n(TermGroundId::default(), n), ctx)
    }
    pub fn from(source: Source, idvec: impl Iterator<Item = TermGroundId>, ctx: &SchedEncoderExt) -> Self {
        let flattenable = FlattenableGround::from(
            idvec,
            ctx.get_source_terms(&source)
                .iter()
                .map(|term| ctx.bounds(term.variable())),
        );
        let assignment = ctx
            .get_source_terms(&source)
            .iter()
            .zip(flattenable.idvec.iter())
            .map(|(&term, id)| TermGround::from(term, *id, ctx))
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();

        Self {
            source,
            flattenable,
            assignment,
        }
    }

    pub fn to_transition_grounding(
        &'a self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoderExt,
    ) -> TransitionTermsGround<'a> {
        let transition_ref = ctx.get_transition(transition_id);
        debug_assert!(self.source == transition_ref.get_source());
        let id = ctx
            .get_transition_terms_positions_in_source_terms(&transition_id)
            .unwrap()
            .map(|i| match i {
                Some(i) => self.flattenable.idvec[i],
                None => TermGroundId::default(),
            });
        TransitionTermsGround::from(transition_id, id, ctx)
    }
    pub fn to_transitions_groundings(
        &'a self,
        ctx: &'a SchedEncoderExt,
    ) -> impl Iterator<Item = TransitionTermsGround<'a>> {
        ctx.get_transitions_of_source(&self.source)
            .map(|tr_id| self.to_transition_grounding(tr_id, ctx))
    }

    pub fn contains(&self, transition_grounding: &TransitionTermsGround, ctx: &SchedEncoderExt) -> bool {
        debug_assert!(self.source == transition_grounding.transition_ref.get_source());

        ctx.get_transition_terms_positions_in_source_terms(&transition_grounding.transition_id)
            .unwrap()
            .enumerate()
            .filter_map(|(j, i)| i.map(|i| (j, i)))
            .all(|(j, i)| self.get_assignment()[j] == transition_grounding.get_assignment()[i])
    }

    pub fn absurd(&self, _ctx: &SchedEncoderExt) -> bool {
        false // TODO FIXME 
    }
}

pub struct TransitionTermsGroundIter<'a> {
    ctx: &'a SchedEncoderExt<'a>,
    // transition_id: Transition_id,
    current: Option<TransitionTermsGround<'a>>,
    is_started: bool,
}

impl<'a> TransitionTermsGroundIter<'a> {
    pub fn new(transition_id: TransitionId, ctx: &'a SchedEncoderExt) -> Self {
        Self {
            ctx,
            // transition_id,
            current: Some(TransitionTermsGround::default(transition_id, ctx)),
            is_started: false,
        }
    }
}

impl<'a> StreamingIterator for TransitionTermsGroundIter<'a> {
    type Item = TransitionTermsGround<'a>;

    fn advance(&mut self) {
        if !self.is_started {
            self.is_started = true;
            return;
        }
        let Some(current) = &mut self.current else { return };
        if current.flattenable.idvec.is_empty() {
            self.current = None;
            return;
        }
        let mut i = current.flattenable.idvec.len() - 1;
        loop {
            if current.flattenable.idvec[i].0 == current.flattenable.dims[i] - 1 {
                if i == 0 {
                    self.current = None;
                    return;
                }
                current.flattenable.idvec[i] = TermGroundId::default();
                i -= 1;
            } else {
                current.flattenable.idvec[i].0 += 1;
                current.flattenable.update_id();
                current.update_assignment(self.ctx);
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}

pub struct SourceTermsGroundIter<'a> {
    ctx: &'a SchedEncoderExt<'a>,
    // source: SourceId,
    current: Option<SourceTermsGround>,
    is_started: bool,
}

impl<'a> SourceTermsGroundIter<'a> {
    pub fn new(source: Source, ctx: &'a SchedEncoderExt) -> Self {
        Self {
            ctx,
            // source,
            current: Some(SourceTermsGround::default(source, ctx)),
            is_started: false,
        }
    }
}

impl<'a> StreamingIterator for SourceTermsGroundIter<'a> {
    type Item = SourceTermsGround;

    fn advance(&mut self) {
        if !self.is_started {
            self.is_started = true;
            if !self.current.as_ref().unwrap().absurd(self.ctx) {
                return;
            }
        }
        let Some(current) = &mut self.current else { return };
        if current.flattenable.idvec.is_empty() {
            self.current = None;
            return;
        }
        let mut i = current.flattenable.idvec.len() - 1;
        loop {
            if current.flattenable.idvec[i].0 == current.flattenable.dims[i] - 1 {
                if i == 0 {
                    self.current = None;
                    return;
                }
                current.flattenable.idvec[i] = TermGroundId::default();
                i -= 1;
            } else {
                current.flattenable.idvec.get_mut(i).unwrap().0 += 1;
                if !current.absurd(self.ctx) {
                    current.flattenable.update_id();
                    current.update_assignment(self.ctx);
                    return;
                }
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}
