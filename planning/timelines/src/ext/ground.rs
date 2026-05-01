use aries::core::views::Term;
use aries::prelude::{DomainsExt, IntCst};
pub use aries::utils::StreamingIterator;

use smallvec::SmallVec;

use crate::IntTerm;
use crate::ext::{SchedEncoderExt, Source, Transition};

impl Transition {
    pub fn iter_groundings(&self, ctx: &SchedEncoderExt) -> impl StreamingIterator<Item = TransitionTermsGround> {
        TransitionTermsGroundIter::new(*self, ctx)
    }
}

impl SchedEncoderExt {
    pub fn iter_transition_groundings(
        &self,
        transition: Transition,
    ) -> impl StreamingIterator<Item = TransitionTermsGround> {
        TransitionTermsGroundIter::new(transition, self)
    }

    pub fn iter_source_groundings(&self, source: Source) -> impl StreamingIterator<Item = SourceTermsGround> {
        SourceTermsGroundIter::new(source, self).filter(|src_gr| !src_gr.absurd(self))
    }

    pub fn iter_source_groundings_containing_transition_grounding(
        &self,
        transition_grounding: &TransitionTermsGround,
    ) -> impl StreamingIterator<Item = SourceTermsGround> {
        let source = transition_grounding.transition.get_source(&self.main);
        self.iter_source_groundings(source)
            .filter(move |src_gr| src_gr.contains(transition_grounding, self))
    }
}

struct FlattenableGround<Id: From<usize>, const N: usize> {
    pub idvec: SmallVec<[TermGroundId; N]>,
    dims: SmallVec<[usize; N]>,
    pub id: Id,
}
impl<Id: From<usize>, const N: usize> FlattenableGround<Id, N> {
    fn from(idvec: impl Iterator<Item = TermGroundId>, dims: impl Iterator<Item = usize>) -> Self {
        let idvec = idvec.collect::<SmallVec<_>>();
        let dims = dims.collect::<SmallVec<_>>();
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

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TermGroundId(pub usize);

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
        Self::from(term, TermGroundId::default(), ctx)
    }
    pub fn from(term: IntTerm, id: TermGroundId, ctx: &SchedEncoderExt) -> Self {
        debug_assert!({
            let (lb_inner, ub_inner) = ctx.bounds(term.variable());
            id.0 <= usize::try_from(ub_inner - lb_inner + 1).unwrap()
        });
        Self { term, id }
    }
    pub fn assignment(&self, ctx: &SchedEncoderExt) -> IntCst {
        let (lb_inner, _) = ctx.bounds(self.term.variable());
        let x = IntCst::try_from(self.id.0).unwrap() + lb_inner;
        self.term.cst() + self.term.factor() * x
    }
}

pub struct TransitionTermsGround {
    pub transition: Transition,
    flattenable: FlattenableGround<TransitionTermsGroundId, 6>,
    assignment: SmallVec<[IntCst; 6]>,
}
impl TransitionTermsGround {
    pub fn idvec(&self) -> &[TermGroundId] {
        &self.flattenable.idvec
    }
    pub fn id(&self) -> TransitionTermsGroundId {
        self.flattenable.id
    }

    fn update_assignment(&mut self, ctx: &SchedEncoderExt) {
        self.assignment = self
            .to_term_groundings(ctx)
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();
    }
    pub fn assignment(&self) -> &[IntCst] {
        &self.assignment
    }
    pub fn to_term_groundings(&self, ctx: &SchedEncoderExt) -> impl Iterator<Item = TermGround> {
        self.transition
            .get_terms(&ctx.main)
            .iter()
            .zip(self.flattenable.idvec.iter())
            .map(|(term, id)| TermGround::from(term, *id, ctx))
    }

    pub fn default(transition: Transition, ctx: &SchedEncoderExt) -> Self {
        let n = transition.get_terms(&ctx.main).len();
        Self::from(transition, std::iter::repeat_n(TermGroundId::default(), n), ctx)
    }
    pub fn from(transition: Transition, idvec: impl Iterator<Item = TermGroundId>, ctx: &SchedEncoderExt) -> Self {
        let flattenable = FlattenableGround::from(
            idvec,
            transition.get_terms(&ctx.main).iter().map(|term| {
                let (lb_inner, ub_inner) = ctx.bounds(term.variable());
                usize::try_from(ub_inner - lb_inner + 1).unwrap()
            }),
        );
        let assignment = transition
            .get_terms(&ctx.main)
            .iter()
            .zip(flattenable.idvec.iter())
            .map(|(term, id)| TermGround::from(term, *id, ctx))
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();

        Self {
            transition,
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

impl SourceTermsGround {
    pub fn idvec(&self) -> &[TermGroundId] {
        &self.flattenable.idvec
    }
    pub fn id(&self) -> SourceTermsGroundId {
        self.flattenable.id
    }

    fn update_assignment(&mut self, ctx: &SchedEncoderExt) {
        self.assignment = self
            .to_term_groundings(ctx)
            .map(|term_grounding| term_grounding.assignment(ctx))
            .collect();
    }
    pub fn assignment(&self) -> &[IntCst] {
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
            ctx.get_source_terms(&source).iter().map(|term| {
                let (lb_inner, ub_inner) = ctx.bounds(term.variable());
                usize::try_from(ub_inner - lb_inner + 1).unwrap()
            }),
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

    pub fn to_transition_grounding(&self, transition: Transition, ctx: &SchedEncoderExt) -> TransitionTermsGround {
        debug_assert!(self.source == transition.get_source(&ctx.main));
        let id = ctx
            .transitions
            .get_transition_terms_positions_in_source_terms(&transition)
            .unwrap()
            .map(|i| match i {
                Some(i) => self.flattenable.idvec[i],
                None => TermGroundId::default(),
            });
        TransitionTermsGround::from(transition, id, ctx)
    }
    pub fn to_transitions_groundings(&self, ctx: &SchedEncoderExt) -> impl Iterator<Item = TransitionTermsGround> {
        ctx.transitions
            .get_for_source(&self.source)
            .map(|&tr| self.to_transition_grounding(tr, ctx))
    }

    pub fn contains(&self, transition_grounding: &TransitionTermsGround, ctx: &SchedEncoderExt) -> bool {
        debug_assert!(self.source == transition_grounding.transition.get_source(&ctx.main));

        ctx.transitions
            .get_transition_terms_positions_in_source_terms(&transition_grounding.transition)
            .unwrap()
            .enumerate()
            .filter_map(|(j, i)| i.map(|i| (j, i)))
            .all(|(j, i)| self.flattenable.idvec[j] == transition_grounding.flattenable.idvec[i])
    }

    pub fn absurd(&self, _ctx: &SchedEncoderExt) -> bool {
        false // TODO FIXME 
    }
}

pub struct TransitionTermsGroundIter<'a> {
    ctx: &'a SchedEncoderExt,
    // transition: Transition,
    current: Option<TransitionTermsGround>,
    is_started: bool,
}

impl<'a> TransitionTermsGroundIter<'a> {
    pub fn new(transition: Transition, ctx: &'a SchedEncoderExt) -> Self {
        Self {
            ctx,
            // transition,
            current: Some(TransitionTermsGround::default(transition, ctx)),
            is_started: false,
        }
    }
}

impl<'a> StreamingIterator for TransitionTermsGroundIter<'a> {
    type Item = TransitionTermsGround;

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
    ctx: &'a SchedEncoderExt,
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
