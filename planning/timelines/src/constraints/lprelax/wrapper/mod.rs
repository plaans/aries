mod lp_highs;
mod traits;

use aries_solver::backtrack::Backtrack;
use traits::LpRelaxReasonerWrapperTraitInner;
pub(crate) use traits::{LpRelaxReasonerWrapped, LpRelaxReasonerWrapperTrait};

#[derive(Clone)]
pub struct LpRelaxReasonerWrapper<T: aries_solver::reasoners::Theory> {
    _phantom: std::marker::PhantomData<fn() -> T>,

    lprelax_encoder_cached: Option<(crate::constraints::lprelax::LpRelaxEncoder, crate::Domains)>,
    ctx: crate::encoder::SchedEncoder,

    num_assumptions: usize,

    encoding_built: bool,
    num_events: u32,
    propagation_calls: usize,
}

impl<T: aries_solver::reasoners::Theory> LpRelaxReasonerWrapper<T> {
    fn build_encoder(&mut self, doms: &crate::Domains) {
        debug_assert!(!self.encoding_built);
        debug_assert!(self.lprelax_encoder_cached.is_none());

        println!(
            "|- Building Lp *encoder* after {} propagation calls (decision level {:?}, num events: {:?})",
            self.propagation_calls,
            doms.current_decision_level(),
            doms.num_events()
        );

        let encoder = crate::constraints::lprelax::LpRelaxEncoder::with_transitions_from(&self.ctx);
        let pre_assumption_doms = doms.clone();

        self.lprelax_encoder_cached = Some((encoder, pre_assumption_doms));
    }
}
