pub mod ground;
pub mod lprelax;
pub mod transitions;

use std::collections::BTreeSet;

use crate::{IntTerm, encoder::SchedEncoder};

use aries::{
    core::views::Term,
    prelude::{DomainsExt, IntCst},
};

pub(crate) use ground::*;
pub(crate) use transitions::*;

pub struct SchedEncoderExt {
    pub(crate) transitions: Transitions,
    pub(crate) empty_source_terms: Vec<IntTerm>,
}
impl SchedEncoderExt {
    pub fn new(sched_encoder: &SchedEncoder) -> Self {
        let empty_source_terms = find_empty_source_terms(sched_encoder);
        Self {
            transitions: Transitions::from(sched_encoder, &empty_source_terms),
            empty_source_terms,
        }
    }
}

fn find_empty_source_terms(ctx: &SchedEncoder) -> Vec<IntTerm> {
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

pub(crate) fn terms_values_from_ids<'a>(
    terms: impl Iterator<Item = &'a IntTerm>,
    ids: &[usize],
    ctx: &'a SchedEncoder,
) -> Vec<IntCst> {
    terms
        .enumerate()
        .map(|(i, term)| term_value_from_id(term, ids[i], ctx).unwrap())
        .collect()
}

pub(crate) fn term_value_from_id(term: &IntTerm, id: usize, ctx: &SchedEncoder) -> Option<IntCst> {
    let (lb, ub) = ctx.bounds(term.variable());
    (id < usize::try_from(ub - lb + 1).unwrap())
        .then(|| term.cst() + lb + term.factor() * IntCst::try_from(id).unwrap())
}
