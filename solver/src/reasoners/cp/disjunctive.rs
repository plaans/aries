use hashbrown::HashSet;
use itertools::Itertools;

use crate::core::state::{Explanation, RangeDomain};
use crate::prelude::*;

use crate::reasoners::Contradiction;
use crate::reasoners::cp::{DynPropagator, UserPropagator};
use crate::{core::state::Term, reasoners::cp::Propagator};

mod theta_tree;

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
    tasks: Vec<Task>,
}

impl NoOverlap {
    pub fn new(tasks: impl IntoIterator<Item = Task>) -> Self {
        Self {
            tasks: tasks.into_iter().collect(),
        }
    }
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

    fn propagate(
        &self,
        domains: &mut Domains,
        _cause: crate::core::state::Cause,
    ) -> Result<(), crate::reasoners::Contradiction> {
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
                use theta_tree::ExplanationItem::*;
                let t = match &i {
                    EstGeq(t, _) | DurationGeq(t, _) | LctLeq(t, _) => t,
                };
                contradiction.push(self.tasks[*t].presence);
                let lit = match i {
                    theta_tree::ExplanationItem::EstGeq(t, v) => self.tasks[t].start.ge_lit(v),
                    theta_tree::ExplanationItem::DurationGeq(t, v) => self.tasks[t].duration.ge_lit(v),
                    theta_tree::ExplanationItem::LctLeq(t, v) => self.tasks[t].end.le_lit(v),
                };
                contradiction.push(lit);
            }

            return Err(Contradiction::Explanation(contradiction));
        }
        Ok(())
    }

    fn explain(
        &self,
        _literal: crate::core::Lit,
        _state: &crate::core::state::DomainsSnapshot,
        _out_explanation: &mut crate::core::state::Explanation,
    ) {
        // not reachable because the overload does not update any value, it just detects a conflict and returns an explanation immediately
        unreachable!()
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
