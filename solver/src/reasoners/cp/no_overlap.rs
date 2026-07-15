use std::fmt::Debug;

use hashbrown::HashSet;
use itertools::Itertools;

use crate::core::state::{DomainsSnapshot, Explanation, InvalidUpdate};
use crate::core::views::{Boundable, Dom, Term, VarView};
use crate::prelude::*;

use crate::reasoners::cp::no_overlap::neg_iatom::PMIAtom;
use crate::reasoners::cp::propagator::justified::*;
use crate::reasoners::cp::{DynPropagator, UserPropagator};

mod neg_iatom;
mod theta_lambda_tree;

/// Defines the level of propagation enforced.
///
/// All propagation algorithms are based on the ThetaLambdaTree from Petr Vilim's thesis.
/// Ref: "Extension of O(n log n) filtering algorithms for the unary resource constraint to optional activities"
///
/// Notable adaptations are with respect to explanations and propagation of the bounds of optional intervals in edge-finding.
///
/// ```
/// use aries_solver::reasoners::cp::no_overlap::PropagatorKind::*;
/// assert!(Overload < OverloadWithOptional);
/// assert!(OverloadWithOptional < EdgeFinding);
/// ```
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropagatorKind {
    /// No propagation at all. Requires redundant constraints (e.g. explicit precedences)
    None,
    /// Overload checking on all tasks known to be present.
    /// This propagator would detect a conflict if a subset of tasks result in an overload.
    Overload,
    /// Same as [`PropagatorKind::Overload`] but in addition deactivates any optional task whose presence would result in an Overload.
    OverloadWithOptional,
    /// Performs edge finding, tightening only the *upper* bounds of task detected to be after a set of others.
    EdgeFindingUpper,
    /// Performs edge finding, tightening the upper/lower bounds of tasks detected to be after/before  a set of others (default).
    #[default]
    EdgeFinding,
}
impl std::str::FromStr for PropagatorKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use PropagatorKind::*;
        match s {
            "none" => Ok(None),
            "overload" => Ok(Overload),
            "overload-opt" => Ok(OverloadWithOptional),
            "edge-finding-upper" | "efu" => Ok(EdgeFindingUpper),
            "edge-finding" | "ef" => Ok(EdgeFinding),
            _ => Err(format!(
                "Invalid option '{s}', accepted: 'none', 'overload', 'overload-opt', 'edge-finding-upper', 'edge-finding'"
            )),
        }
    }
}

/// Representation of a task in the no-overlap constraint.
///
/// Each task has a variable (possibly constant) denoting its start, duration and end time, and a presence
/// literal (possibly tautological if the that is mandatory).
///
/// The presence literal must imply the presence of all all other variables.
#[derive(Debug, Clone)]
pub struct Task<IntVar> {
    start: IntVar,
    duration: IntVar,
    end: IntVar,
    presence: Lit,
}

impl<IntVar> Task<IntVar> {
    pub fn new(start: impl Into<IntVar>, duration: impl Into<IntVar>, end: impl Into<IntVar>, presence: Lit) -> Self {
        Self {
            start: start.into(),
            duration: duration.into(),
            end: end.into(),
            presence,
        }
    }
}

/// No-overlap propagator that propagates the presence, earliest start and latest end time of a set of tasks.
///
/// The propagator implements the algorithms for overload checking and edge-finding, the level of propagation should be set
/// with [`NoOverlapPropagator::with_kind`] method.
///
/// Important note: the propagator should be considered as an additional (redundant) one on top of disjunctive precedences that should
/// be posted independently.
#[derive(Debug, Clone)]
pub struct NoOverlapPropagator<IntVar> {
    pub(crate) kind: PropagatorKind,
    pub(crate) tasks: Vec<Task<IntVar>>,
}

impl<IntVar> NoOverlapPropagator<IntVar>
where
    IntVar: VarView<Value = IntCst> + Copy + Boundable<Value = IntCst>,
{
    /// Creates a new NoOverlap constraint from a set of intervals.
    pub fn new(tasks: impl IntoIterator<Item = Task<IntVar>>) -> Self {
        Self {
            kind: PropagatorKind::default(),
            tasks: tasks.into_iter().collect(),
        }
    }

    /// Sets the propagation level of the constraint.
    pub fn with_kind(mut self, kind: PropagatorKind) -> Self {
        self.kind = kind;
        self
    }

    /// compute the bounds of the activities to place in the tree, ignoring any activity known to be absent
    /// for efficiency, we extract them once from the domains and place their values directly in the tree
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

    #[cfg(not(debug_assertions))]
    fn check_correctness(&self, _domains: &impl Dom, _just: &Justification) {
        // nothing
    }

    /// Checks the correctness of a justification/inference. This has a large overhead and only enabled with debug assertions
    #[cfg(debug_assertions)]
    fn check_correctness(&self, domains: &impl Dom, just: &Justification) {
        //println!("justification: {just:?}");
        match just {
            Justification::Overload { est, lct } => {
                let mut dur = 0;
                for t in self.activities(domains) {
                    if !t.optional && t.est >= *est && t.lct <= *lct {
                        dur += t.p;
                    }
                }
                assert!(est + dur > *lct)
            }
            Justification::OptOverload { est, lct, opt_activity } => {
                let opt = &self.tasks[*opt_activity];
                assert!(*est <= domains.lb(opt.start) && domains.ub(opt.end) <= *lct);
                let mut dur = domains.lb(opt.duration);
                for t in self.activities(domains) {
                    if !t.optional && t.est >= *est && t.lct <= *lct {
                        dur += t.p;
                    }
                }
                assert!(est + dur > *lct)
            }
            &Justification::EdgeFinding {
                est_theta_lambda,
                est_theta,
                lct_theta,
                ect_theta,
                activity,
            } => {
                let opt = &self.tasks[activity];
                let est_i = domains.lb(opt.start);
                let dur_i = domains.lb(opt.duration);
                if est_i >= ect_theta {
                    return; // no-op
                }
                let theta = self
                    .activities(domains)
                    .iter()
                    .cloned()
                    .filter(|t| t.id != activity && !t.optional && t.est >= est_theta && t.lct <= lct_theta)
                    .collect_vec();
                let dur_theta = if theta.is_empty() {
                    0
                } else {
                    let est_theta = theta.iter().map(|t| t.est).min().unwrap();
                    let dur_theta: IntCst = theta.iter().map(|t| t.p).sum();
                    let lct_theta_computed = theta.iter().map(|t| t.lct).max().unwrap();
                    assert!(lct_theta >= lct_theta_computed);
                    let ect_theta_computed = est_theta + dur_theta;
                    assert!(ect_theta_computed >= ect_theta, "{ect_theta_computed} >= {ect_theta}");
                    dur_theta
                };
                let theta_prime = self
                    .activities(domains)
                    .iter()
                    .cloned()
                    .filter(|t| {
                        t.id != activity
                            && !t.optional
                            && t.est >= est_theta_lambda
                            && t.est < est_theta
                            && t.lct <= lct_theta
                    })
                    .collect_vec();
                // println!("theta: \n - {}", theta.iter().map(|t| format!("{t:?}")).join("\n  - "));
                // println!(
                //     "theta': \n - {}",
                //     theta_prime.iter().map(|t| format!("{t:?}")).join("\n  - ")
                // );
                let dur_theta_prime: IntCst = theta_prime.iter().map(|t| t.p).sum();
                let ect_theta_i = est_theta_lambda + dur_theta + dur_theta_prime + dur_i;
                assert!(
                    ect_theta_i > lct_theta,
                    "est_theta: {est_theta}\nest_theta_lambda: {est_theta_lambda}\nest_i: {est_i}\ndur_theta: {dur_theta}\ndur_theta': {dur_theta_prime}\n dur_i: {dur_i}\n Theta: {theta:#?}"
                );
            }
        }
    }

    /// Returns all activities that fit within the [est, lct] interval
    fn activities_inside<'a>(
        &'a self,
        est: IntCst,
        lct: IntCst,
        domains: &'a impl Dom,
    ) -> impl Iterator<Item = (usize, &'a Task<IntVar>)> {
        self.tasks.iter().enumerate().filter(move |(_id, t)| {
            domains.entails(t.presence) && domains.lb(t.start) >= est && domains.ub(t.end) <= lct
        })
    }

    /// Enforce the literals entailed by the given inference.
    fn post_inference(
        &self,
        inference: Justification,
        dom: &mut DomainsAndJustifications<Justification>,
    ) -> Result<(), InvalidUpdate> {
        self.check_correctness(dom, &inference);
        let lit = match &inference {
            Justification::Overload { .. } => Lit::FALSE,
            Justification::OptOverload { opt_activity, .. } => !self.tasks[*opt_activity].presence,
            Justification::EdgeFinding {
                ect_theta, activity, ..
            } => self.tasks[*activity].start.geq(*ect_theta),
        };
        dom.set(lit, inference.clone()).map(|_| ())?;

        Ok(())
    }
}

impl NoOverlapPropagator<VarCst> {
    /// Creates a reversed view of the propagator, which when propagated, will operate on the lower bound.
    /// An interval `(s, d, e)` become the interval `(-e, d, -s)`
    fn reversed(&self) -> NoOverlapPropagator<PMIAtom> {
        let p = |i| PMIAtom::Plus(i);
        let m = |i| PMIAtom::Minus(i);
        NoOverlapPropagator::new(
            self.tasks
                .iter()
                .map(|t| Task::new(m(t.end), p(t.duration), m(t.start), t.presence)),
        )
        .with_kind(self.kind)
    }
}

/// [`UserPropagator`] is only implemented for `IAtom` because we need to be able to reverse the propagator (and we don't have a trait easily capturing that).
impl UserPropagator for NoOverlapPropagator<VarCst> {
    fn get_propagators(&self) -> Vec<super::DynPropagator> {
        let mut propagators = vec![DynPropagator::from(PropagatorWithJustifications::new(self.clone()))];
        if self.kind >= PropagatorKind::EdgeFinding {
            // below this propagation level, there is either no-edge finding or the edge-finding only operates on upper bounds and thus does not required the reversed view.
            propagators.push(DynPropagator::from(PropagatorWithJustifications::new(self.reversed())));
        }
        propagators
    }

    fn satisfied(&self, sol: &Solution) -> bool {
        // maximum intervals of all possibly present tasks
        let itvs = self
            .tasks
            .iter()
            .filter_map(|t| {
                if sol.entails(!t.presence) {
                    None
                } else {
                    Some((sol.eval(t.start).unwrap(), sol.eval(t.end).unwrap()))
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

#[derive(Debug, Clone)]
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
    /// The set of compulsory activities within [est_theta_lambda, lct_theta] are such that
    /// the flagged activity (compulsory or optional) must be placed after
    /// the compulsory activities in [est_theta, lct_theta]
    EdgeFinding {
        est_theta: IntCst,
        lct_theta: IntCst,
        ect_theta: IntCst,
        activity: usize,
        est_theta_lambda: IntCst,
    },
}

impl<IntVar> JustifiedPropagator<Justification> for NoOverlapPropagator<IntVar>
where
    IntVar: VarView<Value = IntCst> + Copy + Send + Sync + Debug + Term + Boundable<Value = IntCst> + 'static,
{
    fn setup(&self, id: super::propagator::PropagatorId, context: &mut crate::reasoners::cp::Watches) {
        // TODO: use a different API for setup
        let mut vars = HashSet::with_capacity(64);
        for t in &self.tasks {
            vars.insert(t.presence.variable());
            vars.insert(t.start.variable());
            vars.insert(t.duration.variable());
            vars.insert(t.end.variable());
        }
        for var in vars {
            context.add_watch(var, id);
        }
    }

    fn propagate(&mut self, domains: &mut DomainsAndJustifications<Justification>) -> Result<(), InvalidUpdate> {
        if self.kind <= PropagatorKind::None {
            return Ok(());
        }
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
                self.post_inference(expl, domains)?;
                unreachable!("Setting an overload always errors and shortcircuits");
            }
            if self.kind >= PropagatorKind::OverloadWithOptional {
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
        }

        // at this point, we have finished the Overload detection (with and without optional activities)
        // - there is no overload between compulsory activities
        // - any overloading optional activity has been flagged as absent and removed from the tree
        // - all other activities are in the tree, white for compulsory and grey for optionals

        if self.kind >= PropagatorKind::EdgeFindingUpper {
            let mut queue = tree
                .tasks()
                .filter(|t| !tree.task(*t).optional)
                .map(|a| (a, tree.task(a).lct))
                .collect_vec();
            queue.sort_unstable_by_key(|(_a, lct)| *lct);

            while let Some((j, lct_j)) = queue.pop() {
                debug_assert!(tree.is_white(j));
                debug_assert_eq!(tree.lct_theta(), lct_j);
                if tree.ect_theta() > lct_j {
                    unreachable!("Overload: should have been detected when building up the tree")
                }
                while tree.ect_theta_lambda() > lct_j {
                    //println!("================= PROPAGATION START =================================");
                    //tree.display();
                    let culprit = tree.cause_of_ect_theta_lambda();
                    debug_assert!(tree.is_grey(culprit));
                    buff.push(Justification::EdgeFinding {
                        est_theta_lambda: tree.est_theta_lambda(),
                        est_theta: tree.est_theta(),
                        lct_theta: lct_j,
                        ect_theta: tree.ect_theta(),
                        activity: tree.task(culprit).id,
                    });
                    //println!("\n\nIN PROPAGATION CHECK");
                    self.check_correctness(domains, buff.last().unwrap());
                    //println!("================= PROPAGATION EDN =================================");

                    tree.remove(culprit);
                }
                tree.make_grey(j);
            }
        }

        for inference in buff {
            self.post_inference(inference, domains)?;
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
        let activities_inside = |est: IntCst, lct: IntCst| self.activities_inside(est, lct, domains);

        match justification {
            Justification::Overload { est, lct } => {
                // the given interval is to small to fit all tasks that must be within it
                // TODO: we could minimize the explanation by ignoring short tasks, whose absence does not impact the overloading
                debug_assert!(lit.absurd());
                for (_id, t) in activities_inside(*est, *lct) {
                    out_explanation.push(t.presence);
                    out_explanation.push(t.start.geq(*est));
                    out_explanation.push(t.end.leq(*lct));
                    out_explanation.push(t.duration.geq(domains.lb(t.duration)));
                }
            }
            Justification::OptOverload { est, lct, opt_activity } => {
                let opt = &self.tasks[*opt_activity];
                debug_assert!((!opt.presence).entails(lit));
                out_explanation.push(opt.start.geq(*est));
                out_explanation.push(opt.end.leq(*lct));
                out_explanation.push(opt.duration.geq(domains.lb(opt.duration)));
                // TODO: we could minimize the explanation by ignoring short tasks, whose absence does not impact the overloading
                for (_id, t) in activities_inside(*est, *lct) {
                    out_explanation.push(t.presence);
                    out_explanation.push(t.start.geq(*est));
                    out_explanation.push(t.end.leq(*lct));
                    out_explanation.push(t.duration.geq(domains.lb(t.duration)));
                }
            }
            Justification::EdgeFinding {
                est_theta_lambda,
                lct_theta,
                ect_theta,
                activity,
                est_theta,
            } => {
                let grey = &self.tasks[*activity];
                debug_assert!(grey.start.geq(*ect_theta).entails(lit));
                out_explanation.push(grey.start.geq(*est_theta_lambda));
                out_explanation.push(grey.duration.geq(domains.lb(grey.duration)));
                for (id, t) in activities_inside(*est_theta_lambda, *lct_theta) {
                    if activity == &id {
                        continue;
                    }
                    out_explanation.push(t.presence);
                    out_explanation.push(t.end.leq(*lct_theta));
                    out_explanation.push(t.duration.geq(domains.lb(t.duration)));
                    let est = domains.lb(t.start);
                    if est >= *est_theta {
                        out_explanation.push(t.start.geq(*est_theta));
                    } else {
                        out_explanation.push(t.start.geq(*est_theta_lambda));
                    }
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            use crate::backtrack::Backtrack;
            // check correctness with current domains
            self.check_correctness(domains, justification);
            // create a copy of the domains such that only the facts in the explanation are true
            let mut domains = domains.domains().clone();
            domains.reset();
            for l in out_explanation.literals() {
                domains.set(*l, crate::core::state::Cause::Encoding).unwrap();
            }
            // check with minimal domains
            self.check_correctness(&domains, justification);

            // reproagate from the initial domains and check that it is indeed re-established
            let mut prop = DynPropagator::from(PropagatorWithJustifications::new(self.clone()));
            // println!("before: {:?}", domains.bounds(lit.variable()));
            let res = prop
                .constraint
                .propagate(&mut domains, crate::core::state::Cause::Encoding)
                .is_ok();
            if res {
                // println!("after: {:?} {lit:?}", domains.bounds(lit.variable()));
                assert!(
                    domains.present(lit) == Some(false) || domains.entails(lit),
                    "{justification:#?}"
                );
            }
        }
    }
}
