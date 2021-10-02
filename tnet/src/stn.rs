use crate::theory::{can_propagate, edge_presence, EdgeId, StnConfig, StnTheory, Timepoint, W};
use aries_backtrack::Backtrack;
use aries_model::literals::{Disjunction, Lit};
use aries_model::state::{Cause, Domains, Explainer, Explanation, InferenceCause};
use aries_model::Model;
use aries_solver::{Contradiction, Theory};

#[derive(Clone)]
pub struct Stn {
    pub(crate) stn: StnTheory,
    pub model: Model<String>,
}
impl Stn {
    pub fn new() -> Self {
        let mut model = Model::new();
        let stn = StnTheory::new(model.new_write_token(), StnConfig::default());
        Stn { stn, model }
    }
    pub fn with_config(config: StnConfig) -> Self {
        let mut model = Model::new();
        let stn = StnTheory::new(model.new_write_token(), config);
        Stn { stn, model }
    }

    pub fn add_timepoint(&mut self, lb: W, ub: W) -> Timepoint {
        self.model.new_ivar(lb, ub, "").into()
    }

    pub fn set_lb(&mut self, timepoint: Timepoint, lb: W) {
        self.model.state.set_lb(timepoint, lb, Cause::Decision).unwrap();
    }

    pub fn set_ub(&mut self, timepoint: Timepoint, ub: W) {
        self.model.state.set_ub(timepoint, ub, Cause::Decision).unwrap();
    }

    pub fn add_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> EdgeId {
        self.stn
            .add_reified_edge(Lit::TRUE, source, target, weight, &self.model.state)
    }

    pub fn add_reified_edge(&mut self, literal: Lit, source: Timepoint, target: Timepoint, weight: W) -> EdgeId {
        self.stn
            .add_reified_edge(literal, source, target, weight, &self.model.state)
    }

    pub fn add_optional_true_edge(
        &mut self,
        source: impl Into<Timepoint>,
        target: impl Into<Timepoint>,
        weight: W,
        forward_prop: Lit,
        backward_prop: Lit,
        presence: Option<Lit>,
    ) -> EdgeId {
        self.stn.add_optional_true_edge(
            source,
            target,
            weight,
            forward_prop,
            backward_prop,
            presence,
            &self.model.state,
        )
    }

    pub fn add_inactive_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> Lit {
        let v = self
            .model
            .new_bvar(format!("reif({:?} -- {} --> {:?})", source, weight, target));
        let activation = v.true_lit();
        self.add_reified_edge(activation, source, target, weight);
        activation
    }

    // add delay between optional variables
    pub fn add_delay(&mut self, a: Timepoint, b: Timepoint, delay: W) {
        // edge a <--- -1 --- b
        let a_to_b = can_propagate(&self.model.state, a, b);
        let b_to_a = can_propagate(&self.model.state, b, a);
        let presence = edge_presence(&self.model.state, a, b);
        self.add_optional_true_edge(b, a, -delay, b_to_a, a_to_b, presence);
    }

    pub fn mark_active(&mut self, edge: Lit) {
        self.model.state.decide(edge).unwrap();
    }

    pub fn propagate_all(&mut self) -> Result<(), Contradiction> {
        self.stn.propagate_all(&mut self.model.state)
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
    pub(crate) fn explain_literal(&mut self, literal: Lit) -> Disjunction {
        struct Exp<'a> {
            stn: &'a mut StnTheory,
        }
        impl<'a> Explainer for Exp<'a> {
            fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation) {
                assert_eq!(cause.writer, self.stn.identity.writer_id);
                self.stn.explain(literal, cause.payload, model, explanation);
            }
        }
        let mut explanation = Explanation::new();
        explanation.push(literal);
        self.model
            .state
            .refine_explanation(explanation, &mut Exp { stn: &mut self.stn })
    }
}

impl Default for Stn {
    fn default() -> Self {
        Self::new()
    }
}
