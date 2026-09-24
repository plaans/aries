mod highs;
mod traits;

use aries_solver::{backtrack::Backtrack, reasoners::Theory};

use traits::LpRelaxReasonerWrapperTraitInner;
pub(crate) use traits::{LpRelaxReasonerWrapped, LpRelaxReasonerWrapperTrait};

use crate::{SchedEncoder, constraints::lprelax::LpRelaxEncoder};

#[derive(Clone)]
pub struct LpRelaxReasonerWrapper<T: Theory> {
    _phantom: std::marker::PhantomData<fn() -> T>,

    lprelax_encoder_cached: Option<(LpRelaxEncoder, crate::Domains)>,
    ctx: SchedEncoder,

    num_assumptions: usize,

    encoding_built: bool,
    num_events: u32,
    propagation_calls: usize,
}

impl<T: Theory> LpRelaxReasonerWrapper<T> {
    fn build_encoder(&mut self, doms: &crate::Domains) {
        debug_assert!(!self.encoding_built);
        debug_assert!(self.lprelax_encoder_cached.is_none());

        let time = std::time::Instant::now();

        let encoder = LpRelaxEncoder::with_transitions_from(&self.ctx);
        let pre_assumption_doms = doms.clone();

        println!(
            "|-[LPRELAX]- Built LP *encoder* after {} propagation calls (decision level {:?}, num events: {:?}) in {}s",
            self.propagation_calls,
            doms.current_decision_level(),
            doms.num_events(),
            time.elapsed().as_secs_f64(),
        );

        self.lprelax_encoder_cached = Some((encoder, pre_assumption_doms));
    }
}
