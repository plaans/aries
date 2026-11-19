//! Linear constraints on the number of instances of each action template in a plan.
//!
//! The constraints are derived from the annotations of the model (see
//! [`annotate_fluents`](crate::preprocessing::annotate_fluents) and
//! [`annotate_supports`](crate::preprocessing::annotate_supports)) by independent *generators*. Each constraint
//! records the generator that produced it, an explanation, and the set of plans for which it holds ([`PlanScope`]).
//!
//! The constraints are only produced here: deriving bounds for each template is left to a solver.
//! A simple bound propagation ([`CountConstraints::propagate`]) is provided for reporting and testing purposes.
//!
//! The only variables are the numbers of instances of each action template, `n(a)`.
// TODO: introduce a variable `u(e)` per effect `e` (number of instances of the action of which `e` is the
//       contribution) to express that each instance needs *some* useful effect: `n(a) <= sum_e u(e)` and
//       `u(e) <= capacity(e)`. This would be more precise than the per-template capacity for actions with several
//       useful effects.

mod supports;
mod transitions;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;

use itertools::Itertools;

use crate::*;

/// The set of plans for which a constraint holds. Scopes are ordered from the weakest assumption to the strongest:
/// a constraint holding for a scope holds for all following ones.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlanScope {
    /// Holds for any valid plan.
    AllPlans,
    /// Holds for any valid *non-redundant* plan: a plan where each action contributes to the goals or metric, i.e.
    /// in which each action instance is the last establisher of a value read by a requirement (of another action or
    /// a goal) or by an opaque expression.
    NonRedundant,
    /// Holds for non-redundant plans without a subset of actions that cancel each other (whose removal leaves the plan
    /// valid), e.g. an action whose only contribution is to enable an action that restores the value it consumed, or
    /// a sequence of transitions of a state variable that comes back to a previous value without the intermediate
    /// values being used.
    NonRedundantPlus,
}

impl Display for PlanScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanScope::AllPlans => write!(f, "all"),
            PlanScope::NonRedundant => write!(f, "NR"),
            PlanScope::NonRedundantPlus => write!(f, "NR+"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cmp {
    Le,
    Ge,
    Eq,
}

/// A linear constraint `sum_i c_i * n(a_i) <cmp> rhs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linear {
    pub terms: Vec<(i64, ActionRef)>,
    pub cmp: Cmp,
    pub rhs: i64,
}

impl Linear {
    /// Builds a linear constraint, merging the terms on the same action and removing null ones.
    pub fn new(terms: impl IntoIterator<Item = (i64, ActionRef)>, cmp: Cmp, rhs: i64) -> Self {
        let mut merged: BTreeMap<ActionRef, i64> = BTreeMap::new();
        for (c, a) in terms {
            *merged.entry(a).or_default() += c;
        }
        let terms = merged
            .into_iter()
            .filter(|(_, c)| *c != 0)
            .map(|(a, c)| (c, a))
            .collect();
        Linear { terms, cmp, rhs }
    }

    pub fn eval(&self, counts: &HashMap<ActionRef, i64>) -> bool {
        let lhs: i64 = (self.terms.iter())
            .map(|(c, a)| c * counts.get(a).copied().unwrap_or(0))
            .sum();
        match self.cmp {
            Cmp::Le => lhs <= self.rhs,
            Cmp::Ge => lhs >= self.rhs,
            Cmp::Eq => lhs == self.rhs,
        }
    }
}

impl Display for Linear {
    /// Displays the constraint with positive coefficients on both sides, e.g. `n(a) <= n(b) + n(c) + 2`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let side = |terms: Vec<String>, cst: i64| {
            let mut parts = terms;
            if cst != 0 || parts.is_empty() {
                parts.push(cst.to_string());
            }
            parts.join(" + ")
        };
        let term = |c: i64, a: &ActionRef| {
            if c == 1 {
                format!("n({a})")
            } else {
                format!("{c}*n({a})")
            }
        };
        let lhs = (self.terms.iter())
            .filter(|(c, _)| *c > 0)
            .map(|(c, a)| term(*c, a))
            .collect();
        let rhs = (self.terms.iter())
            .filter(|(c, _)| *c < 0)
            .map(|(c, a)| term(-c, a))
            .collect();
        // constant on the side where it is positive
        let (lc, rc) = if self.rhs >= 0 { (0, self.rhs) } else { (-self.rhs, 0) };
        let cmp = match self.cmp {
            Cmp::Le => "<=",
            Cmp::Ge => ">=",
            Cmp::Eq => "=",
        };
        write!(f, "{} {cmp} {}", side(lhs, lc), side(rhs, rc))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constraint {
    Linear(Linear),
    /// If the action is used at least once, then the linear constraint must hold.
    Implies {
        if_used: ActionRef,
        then: Linear,
    },
}

impl Constraint {
    pub fn eval(&self, counts: &HashMap<ActionRef, i64>) -> bool {
        match self {
            Constraint::Linear(l) => l.eval(counts),
            Constraint::Implies { if_used, then } => {
                counts.get(if_used).copied().unwrap_or(0) == 0 || then.eval(counts)
            }
        }
    }
}

impl Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constraint::Linear(l) => write!(f, "{l}"),
            Constraint::Implies { if_used, then } => write!(f, "n({if_used}) >= 1  =>  {then}"),
        }
    }
}

/// A constraint on the number of action instances, with its provenance.
#[derive(Clone, Debug)]
pub struct CountConstraint {
    pub constraint: Constraint,
    pub scope: PlanScope,
    /// Name of the generator that produced the constraint.
    pub generator: &'static str,
    /// Human-readable justification.
    pub explanation: String,
}

impl Display for CountConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:>3}] {:<17} {}    -- {}",
            self.scope.to_string(),
            self.generator,
            self.constraint,
            self.explanation
        )
    }
}

/// An upper bound, `None` standing for infinity.
pub type Ub = Option<i64>;

/// The constraints on the number of instances of the action templates of a model.
#[derive(Clone, Debug)]
pub struct CountConstraints {
    pub templates: Vec<ActionRef>,
    pub constraints: Vec<CountConstraint>,
}

impl CountConstraints {
    /// Runs all generators on the model, that is expected to be annotated (fluents and supports).
    /// Generators relying on missing annotations produce no constraint.
    pub fn new(model: &Model) -> Self {
        let templates = model.actions.iter().map(|a| a.name.clone()).collect();
        let mut constraints = Vec::new();
        supports::generate(model, &mut constraints);
        transitions::generate(model, &mut constraints);
        // remove duplicates (keeping the first, i.e. the one with the weakest scope since generators emit
        // constraints of weaker scopes first)
        let mut unique: Vec<CountConstraint> = Vec::new();
        for c in constraints {
            if !unique
                .iter()
                .any(|u| u.constraint == c.constraint && u.scope <= c.scope)
            {
                unique.push(c);
            }
        }
        CountConstraints {
            templates,
            constraints: unique,
        }
    }

    /// The constraints that hold under the given scope (i.e. all constraints of this scope or a weaker one).
    pub fn holding_for(&self, scope: PlanScope) -> impl Iterator<Item = &CountConstraint> + '_ {
        self.constraints.iter().filter(move |c| c.scope <= scope)
    }

    /// The constraints (of any scope) violated by a plan with the given number of instances of each template.
    pub fn violations<'a>(&'a self, counts: &HashMap<ActionRef, i64>) -> Vec<&'a CountConstraint> {
        self.constraints.iter().filter(|c| !c.constraint.eval(counts)).collect()
    }

    /// Bounds `[lb, ub]` on the number of instances of each template, obtained by a simple (incomplete) bound
    /// propagation of the constraints holding for the given scope.
    ///
    /// Returns `None` if the propagation detects that no plan of this scope exists.
    pub fn propagate(&self, scope: PlanScope) -> Option<BTreeMap<ActionRef, (i64, Ub)>> {
        let mut bounds: BTreeMap<ActionRef, (i64, Ub)> =
            self.templates.iter().map(|t| (t.clone(), (0, None))).collect();
        let constraints = self.holding_for(scope).collect_vec();
        // bound the number of iterations (bounds may increase slowly through cycles)
        for _ in 0..1000 {
            let mut changed = false;
            for c in &constraints {
                let lin = match &c.constraint {
                    Constraint::Linear(l) => l,
                    Constraint::Implies { if_used, then } if bounds.get(if_used).is_some_and(|b| b.0 >= 1) => then,
                    Constraint::Implies { .. } => continue,
                };
                let (le, ge) = match lin.cmp {
                    Cmp::Le => (true, false),
                    Cmp::Ge => (false, true),
                    Cmp::Eq => (true, true),
                };
                // min and max of `c * n(a)`
                let range = |bounds: &BTreeMap<ActionRef, (i64, Ub)>, c: i64, a: &ActionRef| -> (Ub, Ub) {
                    let (lb, ub) = bounds.get(a).copied().unwrap_or((0, None));
                    if c >= 0 {
                        (Some(c * lb), ub.map(|ub| c * ub))
                    } else {
                        (ub.map(|ub| c * ub), Some(c * lb))
                    }
                };
                for (i, (ci, ai)) in lin.terms.iter().enumerate() {
                    // sums of the minimum and maximum of the other terms
                    let (mut min_others, mut max_others) = (Some(0), Some(0));
                    for (j, (cj, aj)) in lin.terms.iter().enumerate() {
                        if i != j {
                            let (mn, mx) = range(&bounds, *cj, aj);
                            min_others = min_others.zip(mn).map(|(x, y)| x + y);
                            max_others = max_others.zip(mx).map(|(x, y)| x + y);
                        }
                    }
                    let (lb, ub) = bounds[ai];
                    let (mut new_lb, mut new_ub) = (lb, ub);
                    // c_i * n_i <= rhs - min_others   and   c_i * n_i >= rhs - max_others
                    if le && let Some(mn) = min_others {
                        let r = lin.rhs - mn;
                        if *ci > 0 {
                            let v = r.div_euclid(*ci);
                            new_ub = Some(new_ub.map_or(v, |u| u.min(v)));
                        } else {
                            let v = (-r + (-ci) - 1).div_euclid(-ci); // ceil(r / c_i) with c_i < 0
                            new_lb = new_lb.max(v);
                        }
                    }
                    if ge && let Some(mx) = max_others {
                        let r = lin.rhs - mx;
                        if *ci > 0 {
                            let v = (r + ci - 1).div_euclid(*ci); // ceil
                            new_lb = new_lb.max(v);
                        } else {
                            let v = (-r).div_euclid(-ci); // floor(r / c_i) with c_i < 0
                            new_ub = Some(new_ub.map_or(v, |u| u.min(v)));
                        }
                    }
                    if (new_lb, new_ub) != (lb, ub) {
                        if new_ub.is_some_and(|u| u < new_lb) {
                            return None;
                        }
                        bounds.insert(ai.clone(), (new_lb, new_ub));
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        Some(bounds)
    }
}

impl Display for CountConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for c in &self.constraints {
            writeln!(f, "{c}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;

    use crate::pddl::{build_model, input::Input, parser};
    use crate::preprocessing::{ModelTransformation, apply_transformations};

    use super::*;
    use crate::preprocessing::analysis::Dtg;

    /// Loads a problem of the UPF benchmark set, with the default transformations and the analysis annotations.
    pub(crate) fn load(dir: &str) -> Model {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../problems/upf")
            .join(dir);
        let domain = parser::parse_pddl_domain(Input::from_file(&dir.join("domain.pddl")).unwrap()).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_file(&dir.join("problem.pddl")).unwrap()).unwrap();
        let model = build_model(&domain, &problem).unwrap();
        use ModelTransformation::*;
        apply_transformations(
            model,
            &[
                EraseUnusableEffects,
                SplitPredicates,
                LiftPredicates,
                MergeConditions,
                MergeEffectCondition,
                MergeEraseAssign,
                AnnotateTransitions,
                Intify,
                AnnotateFluents,
                AnnotateSupports,
            ],
        )
        .unwrap()
    }

    /// Propagated bounds for the scope, as `name -> "[lb, ub]"`.
    pub(crate) fn bounds(model: &Model, scope: PlanScope) -> BTreeMap<String, String> {
        let constraints = CountConstraints::new(model);
        println!("{constraints}");
        (constraints.propagate(scope).unwrap().into_iter())
            .map(|(a, (lb, ub))| {
                (
                    a.to_string(),
                    format!("[{lb}, {}]", ub.map_or("inf".to_string(), |u| u.to_string())),
                )
            })
            .collect()
    }

    pub(crate) fn expected(table: &[(&str, &str)]) -> BTreeMap<String, String> {
        table.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn display_and_propagation() {
        let a = || Sym::from("a");
        let b = || Sym::from("b");
        let l = Linear::new([(1, a()), (-2, b()), (1, a())], Cmp::Le, 3);
        assert_eq!(l.to_string(), "2*n(a) <= 2*n(b) + 3");
        let l = Linear::new([(1, a()), (-1, b())], Cmp::Ge, -1);
        assert_eq!(l.to_string(), "n(a) + 1 >= n(b)");
        let constraint = |c: Constraint| CountConstraint {
            constraint: c,
            scope: PlanScope::AllPlans,
            generator: "test",
            explanation: String::new(),
        };
        let cs = CountConstraints {
            templates: vec![a(), b()],
            constraints: vec![
                constraint(Constraint::Linear(Linear::new([(1, a())], Cmp::Ge, 2))),
                constraint(Constraint::Implies {
                    if_used: a(),
                    then: Linear::new([(1, b())], Cmp::Ge, 1),
                }),
                constraint(Constraint::Linear(Linear::new([(2, a()), (1, b())], Cmp::Le, 7))),
            ],
        };
        let bounds = cs.propagate(PlanScope::AllPlans).unwrap();
        assert_eq!(bounds[&a()], (2, Some(3)));
        assert_eq!(bounds[&b()], (1, Some(3)));
    }

    #[test]
    fn satellite() {
        let model = load("ipc2002-satellite-time-simple-automatic");
        assert_eq!(
            bounds(&model, PlanScope::NonRedundantPlus),
            expected(&[
                ("calibrate", "[1, 3]"),
                ("switch_off", "[0, 5]"),
                ("switch_on", "[1, 6]"),
                ("take_image", "[3, 3]"),
                ("turn_to", "[0, 36]"),
            ])
        );
    }

    #[test]
    fn rovers() {
        let model = load("ipc2006-rovers-propositional");
        assert_eq!(
            bounds(&model, PlanScope::NonRedundantPlus),
            expected(&[
                ("calibrate", "[1, 1]"),
                ("communicate_image_data", "[1, 1]"),
                ("communicate_rock_data", "[1, 1]"),
                ("communicate_soil_data", "[1, 1]"),
                ("drop", "[1, 2]"),
                ("navigate", "[0, 21]"),
                ("sample_rock", "[1, 1]"),
                ("sample_soil", "[1, 1]"),
                ("take_image", "[1, 1]"),
            ])
        );
    }

    #[test]
    fn crew_planning() {
        let model = load("ipc2008-crew-planning-temporal-satisficing-strips");
        assert_eq!(
            bounds(&model, PlanScope::NonRedundant),
            expected(&[
                ("assemble_sleep_station", "[1, 1]"),
                ("change_filter", "[1, 1]"),
                ("conduct_payload_activity", "[0, 0]"),
                ("exercise", "[1, 2]"),
                ("finish_rpcm", "[1, 1]"),
                ("first_reconfigurate_thermal_loops", "[1, 1]"),
                ("have_meal", "[1, 2]"),
                ("initialize_day", "[1, 2]"),
                ("medical_conference", "[0, 0]"),
                ("post_sleep", "[1, 2]"),
                ("remove_sleep_station", "[1, 1]"),
                ("replace_rpcm", "[1, 1]"),
                ("report_payload_activity_at_deadline", "[0, 0]"),
                ("second_reconfigurate_thermal_loops", "[1, 1]"),
                ("sleep", "[1, 2]"),
            ])
        );
        // the day only moves forward: valid for any plan
        assert_eq!(bounds(&model, PlanScope::AllPlans)["post_sleep"], "[1, 2]");
    }

    #[test]
    fn tpp() {
        // chained transitions on the stock levels (a soundness issue of an earlier prototype)
        let model = load("ipc2006-tpp-propositional");
        assert_eq!(
            bounds(&model, PlanScope::NonRedundantPlus),
            expected(&[
                ("buy", "[1, 1]"),
                ("drive", "[1, 3]"),
                ("load", "[1, 1]"),
                ("unload", "[1, 1]")
            ])
        );
    }

    #[test]
    fn dtgs() {
        let model = load("ipc2008-crew-planning-temporal-satisficing-strips");
        let day = model.env.fluents.get_by_name("currentday").unwrap();
        let dtg = Dtg::all(&model, 100).into_iter().find(|d| d.fluent == day).unwrap();
        assert_eq!(dtg.longest_path(), Some(2));
        assert_eq!(dtg.reachable().len(), 3);
        assert!(dtg.writers.is_empty());

        let model = load("ipc2002-satellite-time-simple-automatic");
        let power = model.env.fluents.get_by_name("power_avail:power_on").unwrap();
        let dtg = Dtg::all(&model, 100).into_iter().find(|d| d.fluent == power).unwrap();
        // the power is either on the satellite or on its single instrument
        assert_eq!(dtg.reachable().len(), 2);
        assert_eq!(dtg.longest_path(), None);
        assert_eq!(dtg.max_simple_path(), 1);
    }
}
