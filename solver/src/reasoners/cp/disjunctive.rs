use hashbrown::HashSet;
use itertools::Itertools;

use crate::core::state::{Cause, DomainsSnapshot, Explanation, InvalidUpdate};
use crate::core::views::Dom;
use crate::prelude::*;

use crate::reasoners::Contradiction;
use crate::reasoners::cp::disjunctive::theta_lambda_tree::TLTree;
use crate::reasoners::cp::disjunctive::theta_tree::ExplanationItem;
use crate::reasoners::cp::trailed::{DomJust, JustifiedPropagator, MutDomExt};
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
        let prop = JustifiedPropagator::new(self.clone());
        DynPropagator::from(prop)
        //DynPropagator::from(self.clone())
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
            .sorted_unstable()
            .collect_vec();

        for ((s1, e1), (s2, e2)) in itvs.iter().tuple_windows() {
            if s1 > e1 || s2 > e2 {
                return false; // malformormed intervals
            }
            debug_assert!(s1 <= s2); // enforced by sorting
            if e1 > s2 {
                return false; // intervals overlap
            }
        }
        true // no possibly overlapping intervals
    }
}

enum Justification {
    Overload {
        est: IntCst,
        lct: IntCst,
    },
    OptOverload {
        est: IntCst,
        lct: IntCst,
        opt_activity: usize,
    },
}

impl super::trailed::JustifiedProp<Justification> for NoOverlap {
    fn setup(&self, id: super::propagator::PropagatorId, context: &mut crate::reasoners::cp::Watches) {
        Propagator::setup(self, id, context); // TODO
    }

    fn propagate(&mut self, domains: &mut DomJust<Justification>) -> Result<(), InvalidUpdate> {
        use theta_lambda_tree::*;
        let mut buff = Vec::new();
        let activities = self.activities(domains);

        // build the theta tree and look for an overloaded subset of activities
        let mut tree = TLTree::init_empty(activities); // TODO: remove clone

        let mut acts = tree.tasks().map(|a| (a, tree.task(a).lct)).collect_vec();
        acts.sort_unstable_by_key(|(_a, lct)| *lct);

        for (i, lct_i) in acts {
            tree.insert(i); // add task to tree (grey if optional)

            if tree.ect_theta() > lct_i {
                // overload of compulsory activities
                let expl = Justification::Overload {
                    est: tree.est_theta(),
                    lct: lct_i,
                };
                self.set(expl, domains)?;
                unreachable!("Setting an overload always errors and shortcircuits");
            }
            while tree.ect_theta_lambda() > lct_i {
                // there is a grey node that, if added, would cause an overload
                // this task is the one that participates in the computation of ECT(Theta, Lambda)
                let opt_overloader = tree.cause_of_ect_theta_lambda();
                // restore feasibility by forcing its absence and removing it from the tree
                buff.push(Justification::OptOverload {
                    est: tree.est_theta_lambda(),
                    lct: lct_i,
                    opt_activity: tree.task(opt_overloader).id,
                });
                tree.remove(opt_overloader);
            }
        }

        for inference in buff {
            self.set(inference, domains)?;
        }
        Ok(())
    }

    fn explain(
        &self,
        lit: Lit,
        justification: &Justification,
        domains: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        // Returns all activities that fit within the [est, lct] interval
        let activities_inside = |est: IntCst, lct: IntCst| {
            self.tasks
                .iter()
                .filter(move |t| domains.entails(t.presence) && domains.lb(t.start) >= est && domains.ub(t.end) <= lct)
        };

        match justification {
            Justification::Overload { est, lct } => {
                // the given interval is to small to fit all tasks that must be within it
                // TODO: we could minimize the explanation by ignoring short tasks, whose absence does not impact the overloading
                debug_assert!(lit.absurd());
                activities_inside(*est, *lct).for_each(|t| {
                    out_explanation.push(t.presence);
                    out_explanation.push(t.start.ge_lit(*est));
                    out_explanation.push(t.end.le_lit(*lct));
                    out_explanation.push(t.duration.ge_lit(domains.lb(t.duration)));
                });
            }
            Justification::OptOverload { est, lct, opt_activity } => {
                let opt = &self.tasks[*opt_activity];
                debug_assert!((!opt.presence).entails(lit));
                out_explanation.push(opt.start.ge_lit(*est));
                out_explanation.push(opt.end.le_lit(*lct));
                out_explanation.push(opt.duration.ge_lit(domains.lb(opt.duration)));
                // TODO: we could minimize the explanation by ignoring short tasks, whose absence does not impact the overloading
                activities_inside(*est, *lct).for_each(|t| {
                    out_explanation.push(t.presence);
                    out_explanation.push(!t.start.ge_lit(*est));
                    out_explanation.push(t.end.le_lit(*lct));
                    out_explanation.push(t.duration.ge_lit(domains.lb(t.duration)));
                });
            }
        }
    }
}

impl NoOverlap {
    fn set(&self, inference: Justification, dom: &mut DomJust<Justification>) -> Result<(), InvalidUpdate> {
        let lit = match &inference {
            Justification::Overload { .. } => Lit::FALSE,
            Justification::OptOverload { opt_activity, .. } => !self.tasks[*opt_activity].presence,
        };
        dom.set(lit, inference).map(|_| ())
    }
}
