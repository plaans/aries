use crate::backtrack::Backtrack;
use crate::core::literals::Disjunction;
use crate::core::state::{Cause, Domains, Explainer, Explanation, InferenceCause};
use crate::core::Lit;
use crate::model::Model;
use crate::reasoners::stn::theory::{StnConfig, StnTheory, Timepoint, W};
use crate::reasoners::{Contradiction, Theory};

#[derive(Clone)]
pub struct Stn {
    pub(crate) stn: StnTheory,
    pub model: Model<String>,
}
impl Stn {
    pub fn new() -> Self {
        let model = Model::new();
        let stn = StnTheory::new(StnConfig::default());
        Stn { stn, model }
    }
    pub fn new_with_config(config: StnConfig) -> Self {
        let model = Model::new();
        let stn = StnTheory::new(config);
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

    pub fn add_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) {
        let valid_edge = self.get_conjunctive_scope(source, target);
        let active_edge = self.model.get_tautology_of_scope(valid_edge);
        debug_assert!(self.model.state.entails(active_edge));
        self.stn
            .add_reified_edge(active_edge, source, target, weight, &self.model.state)
    }

    pub fn add_inactive_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> Lit {
        let valid_edge = self.get_conjunctive_scope(source, target);
        let active_edge = self
            .model
            .new_optional_bvar(valid_edge, format!("reif({source:?} -- {weight} --> {target:?})"))
            .true_lit();

        self.stn
            .add_reified_edge(active_edge, source, target, weight, &self.model.state);
        active_edge
    }

    // add delay between optional variables
    pub fn add_delay(&mut self, a: impl Into<Timepoint>, b: impl Into<Timepoint>, delay: W) {
        self.add_edge(b.into(), a.into(), -delay);
    }

    /// Returns a literal that is true iff both timepoints are present.
    fn get_conjunctive_scope(&mut self, a: Timepoint, b: Timepoint) -> Lit {
        let pa = self.model.state.presence(a);
        let pb = self.model.state.presence(b);
        self.model.get_conjunctive_scope(&[pa, pb])
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
            .clause
    }
}

impl Default for Stn {
    fn default() -> Self {
        Self::new()
    }
}
