use crate::theory::{EdgeId, StnConfig, StnTheory, Timepoint, W};
use aries_backtrack::Backtrack;
use aries_model::bounds::{Bound, Disjunction};
use aries_model::int_model::domains::Domains;
use aries_model::int_model::{Cause, DiscreteModel, Explainer, Explanation, InferenceCause};
use aries_model::Model;
use aries_solver::{Contradiction, Theory};

#[derive(Clone)]
pub struct Stn {
    pub(crate) stn: StnTheory,
    pub model: Model,
}
impl Stn {
    pub fn new() -> Self {
        let mut model = Model::new();
        let stn = StnTheory::new(model.new_write_token(), StnConfig::default());
        Stn { stn, model }
    }

    pub fn add_timepoint(&mut self, lb: W, ub: W) -> Timepoint {
        self.model.new_ivar(lb, ub, "").into()
    }

    pub fn set_lb(&mut self, timepoint: Timepoint, lb: W) {
        self.model.discrete.set_lb(timepoint, lb, Cause::Decision).unwrap();
    }

    pub fn set_ub(&mut self, timepoint: Timepoint, ub: W) {
        self.model.discrete.set_ub(timepoint, ub, Cause::Decision).unwrap();
    }

    pub fn add_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> EdgeId {
        self.stn
            .add_reified_edge(Bound::TRUE, source, target, weight, &self.model)
    }

    pub fn add_reified_edge(&mut self, literal: Bound, source: Timepoint, target: Timepoint, weight: W) -> EdgeId {
        self.stn.add_reified_edge(literal, source, target, weight, &self.model)
    }

    pub fn add_optional_true_edge(
        &mut self,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        forward_prop: Bound,
        backward_prop: Bound,
    ) -> EdgeId {
        self.stn
            .add_optional_true_edge(source, target, weight, forward_prop, backward_prop, &self.model)
    }

    pub fn add_inactive_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> Bound {
        let v = self
            .model
            .new_bvar(format!("reif({:?} -- {} --> {:?})", source, weight, target));
        let activation = v.true_lit();
        self.add_reified_edge(activation, source, target, weight);
        activation
    }

    // add delay between optional variables
    pub fn add_delay(&mut self, a: Timepoint, b: Timepoint, delay: W) {
        fn can_propagate(doms: &Domains, from: Timepoint, to: Timepoint) -> Bound {
            // lit = (from ---> to)    ,  we can if (lit != false) && p(from) => p(to)
            if doms.only_present_with(to, from) {
                Bound::TRUE
            } else if doms.only_present_with(from, to) {
                // to => from, to = true means (from => to)
                doms.presence(to)
            } else {
                panic!()
            }
        }
        // edge a <--- -1 --- b
        let a_to_b = can_propagate(&self.model.discrete.domains, a, b);
        let b_to_a = can_propagate(&self.model.discrete.domains, b, a);
        self.add_optional_true_edge(b, a, -delay, b_to_a, a_to_b);
    }

    pub fn mark_active(&mut self, edge: Bound) {
        self.model.discrete.decide(edge).unwrap();
    }

    pub fn propagate_all(&mut self) -> Result<(), Contradiction> {
        self.stn.propagate_all(&mut self.model.discrete)
    }

    pub fn set_backtrack_point(&mut self) {
        self.model.save_state();
        self.stn.set_backtrack_point();
    }

    pub fn undo_to_last_backtrack_point(&mut self) {
        self.model.restore_last();
        self.stn.undo_to_last_backtrack_point();
    }

    // ------ Private method for testing purposes -------

    #[allow(unused)]
    pub(crate) fn assert_consistent(&mut self) {
        assert!(self.propagate_all().is_ok());
    }

    #[allow(unused)]
    pub(crate) fn assert_inconsistent<X>(&mut self, mut _err: Vec<X>) {
        assert!(self.propagate_all().is_err());
    }

    #[allow(unused)]
    pub(crate) fn explain_literal(&mut self, literal: Bound) -> Disjunction {
        struct Exp<'a> {
            stn: &'a mut StnTheory,
        }
        impl<'a> Explainer for Exp<'a> {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: Bound,
                model: &DiscreteModel,
                explanation: &mut Explanation,
            ) {
                assert_eq!(cause.writer, self.stn.identity.writer_id);
                self.stn.explain(literal, cause.payload, model, explanation);
            }
        }
        let mut explanation = Explanation::new();
        explanation.push(literal);
        self.model
            .discrete
            .refine_explanation(explanation, &mut Exp { stn: &mut self.stn })
    }
}

impl Default for Stn {
    fn default() -> Self {
        Self::new()
    }
}
