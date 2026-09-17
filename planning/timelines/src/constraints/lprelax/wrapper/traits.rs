use aries_solver::backtrack::{Backtrack, DecLvl};
use aries_solver::core::state::{DomainsSnapshot, Explanation, InferenceCause};
use aries_solver::prelude::{Domains, Lit};
use aries_solver::reasoners::{Contradiction, ReasonerId, Theory};

use crate::encoder::SchedEncoder;

pub trait LpRelaxReasonerWrapperTrait: LpRelaxReasonerWrapperTraitInner {
    fn new_wrapped(ctx: SchedEncoder, num_assumptions: usize) -> LpRelaxReasonerWrapped<Self>;
}

pub trait LpRelaxReasonerWrapperTraitInner: Clone + Send + 'static {
    type Thr: Theory + Clone + Default + 'static;

    fn pre_propagate(&mut self, theory: &mut Self::Thr, model: &mut Domains) -> Result<(), Contradiction>;

    fn post_propagate(&mut self, theory: &mut Self::Thr, model: &mut Domains) -> Result<(), Contradiction>;
}

pub struct LpRelaxReasonerWrapped<W: LpRelaxReasonerWrapperTrait> {
    wrapper: W,
    inner_theory: W::Thr,
}

impl<W: LpRelaxReasonerWrapperTrait> LpRelaxReasonerWrapped<W> {
    pub(super) fn new(wrapper: W, inner_theory: W::Thr) -> Self {
        Self { wrapper, inner_theory }
    }
}

impl<W: LpRelaxReasonerWrapperTrait> Theory for LpRelaxReasonerWrapped<W> {
    fn identity(&self) -> ReasonerId {
        self.inner_theory.identity()
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        self.wrapper.pre_propagate(&mut self.inner_theory, model)?;
        self.inner_theory.propagate(model)?;
        self.wrapper.post_propagate(&mut self.inner_theory, model)
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        self.inner_theory.explain(literal, context, model, out_explanation);
    }

    fn print_stats(&self) {
        self.inner_theory.print_stats();
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(Self {
            inner_theory: self.inner_theory.clone(),
            wrapper: self.wrapper.clone(),
        })
    }
}

impl<W: LpRelaxReasonerWrapperTrait> Backtrack for LpRelaxReasonerWrapped<W> {
    fn save_state(&mut self) -> DecLvl {
        self.inner_theory.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.inner_theory.num_saved()
    }

    fn restore_last(&mut self) {
        self.inner_theory.restore_last()
    }
}
