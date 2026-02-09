use hashbrown::HashSet;
use itertools::Itertools;

use crate::core::state::{Cause, DomainsSnapshot, Explanation};
use crate::core::views::Dom;
use crate::prelude::*;

use crate::reasoners::Contradiction;
use crate::reasoners::cp::disjunctive::theta_lambda_tree::TLTree;
use crate::reasoners::cp::disjunctive::theta_tree::ExplanationItem;
use crate::reasoners::cp::{DynPropagator, UserPropagator};
use crate::{core::state::Term, reasoners::cp::Propagator};

mod theta_lambda_tree;
mod theta_tree;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum PropagatorKind {
    /// Overload checking on all tasks known to be present.
    /// This propagator would detect a conflict if a subset of tasks result in an overload.
    Overload,
    /// Same as [`Overload`] but in addition deactivate any optional task whose presence would result in an Overload.
    #[default]
    OverloadWithOptional,
}

#[derive(Debug, Clone)]
pub struct Task {
    start: IAtom,
    duration: IAtom,
    end: IAtom,
    presence: Lit,
}

impl Task {
    pub fn new(start: impl Into<IAtom>, duration: impl Into<IAtom>, end: impl Into<IAtom>, presence: Lit) -> Self {
        Self {
            start: start.into(),
            duration: duration.into(),
            end: end.into(),
            presence,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoOverlap {
    kind: PropagatorKind,
    tasks: Vec<Task>,
}

impl NoOverlap {
    pub fn new(tasks: impl IntoIterator<Item = Task>) -> Self {
        Self {
            kind: PropagatorKind::default(),
            tasks: tasks.into_iter().collect(),
        }
    }
    pub fn kind(mut self, kind: PropagatorKind) -> Self {
        self.kind = kind;
        self
    }
    fn expl_item_to_lit(&self, item: ExplanationItem) -> Lit {
        match item {
            ExplanationItem::Present(t) => self.tasks[t].presence,
            ExplanationItem::Absent(t) => !self.tasks[t].presence,
            ExplanationItem::EstGeq(t, v) => self.tasks[t].start.ge_lit(v),
            ExplanationItem::DurationGeq(t, v) => self.tasks[t].duration.ge_lit(v),
            ExplanationItem::LctLeq(t, v) => self.tasks[t].end.le_lit(v),
        }
    }

    fn propagate_overload(
        &self,
        domains: &mut Domains,
        _cause: crate::core::state::Cause,
    ) -> Result<(), crate::reasoners::Contradiction> {
        debug_assert_eq!(self.kind, PropagatorKind::Overload);
        // compute the bounds of the activities to place in the tree, ignoring any activity to known to be present
        // for efficiency, we extract them once from the domains and place their values directly in the tree
        let acts = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| domains.entails(t.presence))
            .map(|(i, t)| theta_tree::Activity::new(i, domains.lb(t.start), domains.ub(t.end), domains.lb(t.duration)))
            .collect_vec();

        // build the theta tree and look for an overloaded subset of activities
        let mut tree = theta_tree::ThetaTree::init_empty(acts);
        if tree.find_overloaded_subset() {
            // there is an overload, corresponding to the current tasks in the tree
            debug_assert!(!self.satisfied(domains));

            // get a minimal set of overloaded tasks and compute the corresponding explanation
            tree.minimize_overloaded_set();
            let explanation = tree.explain_overload();

            let mut contradiction = Explanation::with_capacity(explanation.len());
            for i in explanation {
                contradiction.push(self.expl_item_to_lit(i));
            }

            return Err(Contradiction::Explanation(contradiction));
        }
        Ok(())
    }
    fn propagate_overload_with_optional(
        &self,
        domains: &mut Domains,
        cause: crate::core::state::Cause,
    ) -> Result<(), crate::reasoners::Contradiction> {
        use theta_lambda_tree::*;
        debug_assert_eq!(self.kind, PropagatorKind::OverloadWithOptional);
        // compute the bounds of the activities to place in the tree, ignoring any activity to known to be present
        // for efficiency, we extract them once from the domains and place their values directly in the tree
        let acts = self.activities(domains);

        // build the theta tree and look for an overloaded subset of activities
        let mut tree = TLTree::init_empty(acts); // TODO: remove clone
        let mut buff = Vec::new();
        match tree.check_overload(&mut buff) {
            PropagationResult::Conflict(conflict_set) => {
                let mut contradiction = Explanation::with_capacity(conflict_set.len());
                for conflict_item in conflict_set {
                    contradiction.push(self.expl_item_to_lit(*conflict_item));
                }
            }
            PropagationResult::Inferences(inferences) => {
                for inferred in inferences {
                    let lit = self.expl_item_to_lit(*inferred);
                    if is_witness(lit) {
                        // TODO: left here to ease debugging
                        println!("inferred: {inferred:?}");
                    }
                    domains.set(lit, cause)?;
                }
            }
        }

        Ok(())
    }
    fn explain_overload_with_optional(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
        debug_assert_eq!(self.kind, PropagatorKind::OverloadWithOptional);
        // the only thing we can propagate is the absence of an optional task
        // First, we find which are the optional tasks that may have been deactivated

        let activities = self
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let optional = if state.entails(!t.presence) {
                    return None; // necessarily absent, ignore
                } else if state.entails(t.presence) {
                    false // non-optional: presence is entailed
                } else if literal.entails(!t.presence) {
                    true // optional and deactivated by the literal we aim to explain
                } else {
                    return None; // optional but not deactivated by our literal
                };
                Some(theta_lambda_tree::Activity::new(
                    i,
                    state.lb(t.start),
                    state.ub(t.end),
                    state.lb(t.duration),
                    optional,
                ))
            })
            .collect_vec();
        if is_witness(literal) {
            dbg!(&activities);
        }
        let mut tree = TLTree::init_empty(activities);
        let (implicants, deactivated) = tree.explain_overload_deactivation();
        debug_assert!(literal.entails(!self.tasks[deactivated].presence));
        for cause in implicants {
            out_explanation.push(self.expl_item_to_lit(*cause));
        }
    }

    // compute the bounds of the activities to place in the tree, ignoring any activity to known to be present
    // for efficiency, we extract them once from the domains and place their values directly in the tree
    fn activities(&self, domains: &impl Dom) -> Vec<theta_lambda_tree::Activity> {
        self.tasks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let optional = if domains.entails(!t.presence) {
                    return None; // necessarily absent, ignore
                } else if domains.entails(t.presence) {
                    false // non-optional: presence is entailed
                } else {
                    true // optional task
                };
                Some(theta_lambda_tree::Activity::new(
                    i,
                    domains.lb(t.start),
                    domains.ub(t.end),
                    domains.lb(t.duration),
                    optional,
                ))
            })
            .collect_vec()
    }
}

const WITNESS: &str = "!l3927_NEVER";
fn is_witness(lit: Lit) -> bool {
    WITNESS == format!("{lit:?}")
}

impl Propagator for NoOverlap {
    fn setup(&self, id: super::PropagatorId, context: &mut super::Watches) {
        let mut vars = HashSet::with_capacity(64);
        for t in &self.tasks {
            vars.insert(t.start.var.variable());
            vars.insert(t.duration.var.variable());
            vars.insert(t.end.var.variable());
        }
        for var in vars {
            context.add_watch(var, id);
        }
    }
    fn propagate(&mut self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        match self.kind {
            PropagatorKind::Overload => self.propagate_overload(domains, cause),
            PropagatorKind::OverloadWithOptional => self.propagate_overload_with_optional(domains, cause),
        }
    }

    fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
        match self.kind {
            PropagatorKind::Overload => unreachable!(), // not reachable because the overload does not update any value, it just detects a conflict and returns an explanation immediately
            PropagatorKind::OverloadWithOptional => {
                self.explain_overload_with_optional(literal, state, out_explanation)
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

impl UserPropagator for NoOverlap {
    fn get_propagator(&self) -> super::DynPropagator {
        DynPropagator::from(self.clone())
    }

    fn satisfied(&self, dom: &Domains) -> bool {
        // maximum intervals of all possibly present tasks
        let itvs = self
            .tasks
            .iter()
            .filter_map(|t| {
                if dom.entails(!t.presence) {
                    None
                } else {
                    Some((dom.lb(t.start), dom.ub(t.end)))
                }
            })
            .collect_vec();
        // Check all intervals to see if any may overlap
        // Naive O(n^2) implementation. An O(n x log(n)) implementation is possible
        // but probably not worth the trouble
        for (i, (s1, e1)) in itvs.iter().enumerate() {
            for (s2, e2) in &itvs[(i + 1)..] {
                if !(e1 <= s2 || e2 <= s1) {
                    return false;
                }
            }
        }
        true // no possibly overlapping intervals
    }
}
