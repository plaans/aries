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

mod numeric;
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

pub use aries_linexpr::ComparisonOp as Cmp;
use aries_linexpr::{CanonicalConstraint, LinExpr, impl_linear_ops};

/// The variable `n(a)`: number of instances of an action template `a` (see [`CountVars`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CountVar(u32);

impl_linear_ops!(CountVar, i64);

/// A linear expression over the count variables.
pub type CountExpr = LinExpr<CountVar, i64>;

/// A linear constraint over the count variables, in canonical form (`sum_i c_i * n(a_i) + k <cmp> 0`).
pub type Linear = CanonicalConstraint<CountVar, i64>;

/// The count variables of the action templates of a model.
#[derive(Clone, Debug)]
pub struct CountVars {
    templates: Vec<ActionRef>,
    index: HashMap<ActionRef, CountVar>,
}

impl CountVars {
    pub fn new(model: &Model) -> Self {
        Self::from_templates(model.actions.iter().map(|a| a.name.clone()).collect())
    }

    pub fn from_templates(templates: Vec<ActionRef>) -> Self {
        let index = (templates.iter().enumerate())
            .map(|(i, a)| (a.clone(), CountVar(i as u32)))
            .collect();
        CountVars { templates, index }
    }

    /// The variable `n(a)` of a template of the model.
    pub fn n(&self, a: &ActionRef) -> CountVar {
        self.index[a]
    }

    /// The template counted by the variable.
    pub fn template(&self, v: CountVar) -> &ActionRef {
        &self.templates[v.0 as usize]
    }

    /// All variables with their template.
    pub fn iter(&self) -> impl Iterator<Item = (CountVar, &ActionRef)> + '_ {
        (self.templates.iter().enumerate()).map(|(i, a)| (CountVar(i as u32), a))
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// The expression `sum_i c_i * n(a_i)` (simplified).
    pub fn sum(&self, terms: impl IntoIterator<Item = (i64, ActionRef)>) -> CountExpr {
        let mut e: CountExpr = terms.into_iter().map(|(c, a)| (self.n(&a), c)).collect();
        e.simplify();
        e
    }

    /// The constraint `sum_i c_i * n(a_i) <cmp> rhs`.
    pub fn linear(&self, terms: impl IntoIterator<Item = (i64, ActionRef)>, cmp: Cmp, rhs: i64) -> Linear {
        self.sum(terms).into_constraint(cmp, rhs).canonical()
    }

    /// Coefficient of `n(a)` in the constraint.
    pub fn coeff(&self, lin: &Linear, a: &ActionRef) -> i64 {
        let v = self.n(a);
        lin.expr().iter().filter(|(x, _)| *x == v).map(|(_, c)| c).sum()
    }

    /// Displays an element (constraint, expression) with the names of the templates.
    pub fn display<'a, T: ?Sized>(&'a self, elem: &'a T) -> Named<'a, T> {
        Named { vars: self, elem }
    }
}

/// An element to be displayed with the names of the templates of its variables.
pub struct Named<'a, T: ?Sized> {
    vars: &'a CountVars,
    elem: &'a T,
}

impl Display for Named<'_, Linear> {
    /// Displays the constraint with positive coefficients on both sides, e.g. `n(a) <= n(b) + n(c) + 2`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lin = self.elem;
        let side = |terms: Vec<String>, cst: i64| {
            let mut parts = terms;
            if cst != 0 || parts.is_empty() {
                parts.push(cst.to_string());
            }
            parts.join(" + ")
        };
        let term = |c: i64, v: CountVar| {
            let a = self.vars.template(v);
            if c == 1 {
                format!("n({a})")
            } else {
                format!("{c}*n({a})")
            }
        };
        let terms = lin.expr().iter().collect_vec();
        let lhs = (terms.iter())
            .filter(|(_, c)| *c > 0)
            .map(|(v, c)| term(*c, *v))
            .collect();
        let rhs = (terms.iter())
            .filter(|(_, c)| *c < 0)
            .map(|(v, c)| term(-c, *v))
            .collect();
        // constant on the side where it is positive
        let k = -lin.expr().constant();
        let (lc, rc) = if k >= 0 { (0, k) } else { (-k, 0) };
        let cmp = match lin.comp_op() {
            Cmp::Eq => "=",
            op => op.symbol(),
        };
        write!(f, "{} {cmp} {}", side(lhs, lc), side(rhs, rc))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constraint {
    Linear(Linear),
    /// If the action is used at least once, then the linear constraint must hold.
    Implies {
        if_used: CountVar,
        then: Linear,
    },
}

impl Constraint {
    /// True if the constraint holds, given the value of each count variable.
    pub fn eval(&self, value: impl Fn(CountVar) -> i64) -> bool {
        match self {
            Constraint::Linear(l) => l.is_satisfied(value),
            Constraint::Implies { if_used, then } => value(*if_used) == 0 || then.is_satisfied(value),
        }
    }
}

impl Display for Named<'_, Constraint> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.elem {
            Constraint::Linear(l) => write!(f, "{}", self.vars.display(l)),
            Constraint::Implies { if_used, then } => {
                let a = self.vars.template(*if_used);
                write!(f, "n({a}) >= 1  =>  {}", self.vars.display(then))
            }
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

impl Display for Named<'_, CountConstraint> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = self.elem;
        write!(
            f,
            "[{:>3}] {:<17} {}    -- {}",
            c.scope.to_string(),
            c.generator,
            self.vars.display(&c.constraint),
            c.explanation
        )
    }
}

/// An upper bound, `None` standing for infinity.
pub type Ub = Option<i64>;

/// The constraints on the number of instances of the action templates of a model.
#[derive(Clone, Debug)]
pub struct CountConstraints {
    pub vars: CountVars,
    pub constraints: Vec<CountConstraint>,
}

impl CountConstraints {
    /// Runs all generators on the model, that is expected to be annotated (fluents and supports).
    /// Generators relying on missing annotations produce no constraint.
    ///
    /// In hierarchical models, actions may be required by the decomposition of tasks without contributing to the
    /// goals: only the constraints holding for all plans are kept.
    pub fn new(model: &Model) -> Self {
        let vars = CountVars::new(model);
        let mut constraints = Vec::new();
        supports::generate(model, &vars, &mut constraints);
        transitions::generate(model, &vars, &mut constraints);
        numeric::generate(model, &vars, &mut constraints);
        let hierarchical = model.task_network.is_some() || model.actions.iter().any(|a| !a.subtasks.is_empty());
        if hierarchical {
            constraints.retain(|c| c.scope == PlanScope::AllPlans);
        }
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
            vars,
            constraints: unique,
        }
    }

    /// The constraints that hold under the given scope (i.e. all constraints of this scope or a weaker one).
    pub fn holding_for(&self, scope: PlanScope) -> impl Iterator<Item = &CountConstraint> + '_ {
        self.constraints.iter().filter(move |c| c.scope <= scope)
    }

    /// The constraints (of any scope) violated by a plan with the given number of instances of each template.
    pub fn violations<'a>(&'a self, counts: &HashMap<ActionRef, i64>) -> Vec<&'a CountConstraint> {
        let value = |v: CountVar| counts.get(self.vars.template(v)).copied().unwrap_or(0);
        (self.constraints.iter())
            .filter(|c| !c.constraint.eval(value))
            .collect()
    }

    /// Bounds `[lb, ub]` on the number of instances of each template, obtained by a simple (incomplete) bound
    /// propagation of the constraints holding for the given scope.
    ///
    /// Returns `None` if the propagation detects that no plan of this scope exists.
    pub fn propagate(&self, scope: PlanScope) -> Option<BTreeMap<ActionRef, (i64, Ub)>> {
        let mut bounds: Vec<(i64, Ub)> = vec![(0, None); self.vars.len()];
        let constraints = self.holding_for(scope).collect_vec();
        // bound the number of iterations (bounds may increase slowly through cycles)
        for _ in 0..1000 {
            let mut changed = false;
            for c in &constraints {
                let lin = match &c.constraint {
                    Constraint::Linear(l) => l,
                    Constraint::Implies { if_used, then } if bounds[if_used.0 as usize].0 >= 1 => then,
                    Constraint::Implies { .. } => continue,
                };
                let (le, ge) = match lin.comp_op() {
                    Cmp::Le => (true, false),
                    Cmp::Ge => (false, true),
                    Cmp::Eq => (true, true),
                };
                let rhs = -lin.expr().constant();
                let terms = lin.expr().iter().collect_vec();
                // min and max of `c * n(a)`
                let range = |bounds: &[(i64, Ub)], v: CountVar, c: i64| -> (Ub, Ub) {
                    let (lb, ub) = bounds[v.0 as usize];
                    if c >= 0 {
                        (Some(c * lb), ub.map(|ub| c * ub))
                    } else {
                        (ub.map(|ub| c * ub), Some(c * lb))
                    }
                };
                for (i, &(vi, ci)) in terms.iter().enumerate() {
                    // sums of the minimum and maximum of the other terms
                    let (mut min_others, mut max_others) = (Some(0), Some(0));
                    for (j, &(vj, cj)) in terms.iter().enumerate() {
                        if i != j {
                            let (mn, mx) = range(&bounds, vj, cj);
                            min_others = min_others.zip(mn).map(|(x, y)| x + y);
                            max_others = max_others.zip(mx).map(|(x, y)| x + y);
                        }
                    }
                    let (lb, ub) = bounds[vi.0 as usize];
                    let (mut new_lb, mut new_ub) = (lb, ub);
                    // c_i * n_i <= rhs - min_others   and   c_i * n_i >= rhs - max_others
                    if le && let Some(mn) = min_others {
                        let r = rhs - mn;
                        if ci > 0 {
                            let v = r.div_euclid(ci);
                            new_ub = Some(new_ub.map_or(v, |u| u.min(v)));
                        } else {
                            let v = (-r + (-ci) - 1).div_euclid(-ci); // ceil(r / c_i) with c_i < 0
                            new_lb = new_lb.max(v);
                        }
                    }
                    if ge && let Some(mx) = max_others {
                        let r = rhs - mx;
                        if ci > 0 {
                            let v = (r + ci - 1).div_euclid(ci); // ceil
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
                        bounds[vi.0 as usize] = (new_lb, new_ub);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        Some(
            (self.vars.iter())
                .map(|(v, a)| (a.clone(), bounds[v.0 as usize]))
                .collect(),
        )
    }
}

impl Display for CountConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for c in &self.constraints {
            writeln!(f, "{}", self.vars.display(c))?;
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

    /// Builds a model from PDDL strings, with the default transformations and the analysis annotations.
    pub(crate) fn from_strings(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let model = build_model(&domain, &problem).unwrap();
        use ModelTransformation::*;
        apply_transformations(
            model,
            &[
                EraseUnusableEffects,
                SplitPredicates,
                LiftPredicates,
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
        let vars = CountVars::from_templates(vec![a(), b()]);
        let l = vars.linear([(1, a()), (-2, b()), (1, a())], Cmp::Le, 3);
        assert_eq!(vars.display(&l).to_string(), "2*n(a) <= 2*n(b) + 3");
        let l = vars.linear([(1, a()), (-1, b())], Cmp::Ge, -1);
        assert_eq!(vars.display(&l).to_string(), "n(a) + 1 >= n(b)");
        // built with operators
        let (na, nb) = (vars.n(&a()), vars.n(&b()));
        assert_eq!(
            vars.display(&(2 * na - nb).leq(4).canonical()).to_string(),
            "2*n(a) <= n(b) + 4"
        );
        let constraint = |c: Constraint| CountConstraint {
            constraint: c,
            scope: PlanScope::AllPlans,
            generator: "test",
            explanation: String::new(),
        };
        let constraints = vec![
            constraint(Constraint::Linear(vars.linear([(1, a())], Cmp::Ge, 2))),
            constraint(Constraint::Implies {
                if_used: na,
                then: vars.linear([(1, b())], Cmp::Ge, 1),
            }),
            constraint(Constraint::Linear(vars.linear([(2, a()), (1, b())], Cmp::Le, 7))),
        ];
        let cs = CountConstraints { vars, constraints };
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
    fn numeric_budget() {
        let domain = "(define (domain d) (:requirements :typing :numeric-fluents)
            (:types truck place)
            (:predicates (at ?t - truck ?p - place) (visited ?p - place))
            (:functions (fuel ?t - truck) (cost))
            (:action move :parameters (?t - truck ?from - place ?to - place)
              :precondition (and (at ?t ?from) (>= (fuel ?t) 2))
              :effect (and (not (at ?t ?from)) (at ?t ?to) (visited ?to) (decrease (fuel ?t) 2) (increase (cost) 1))))";
        let problem = "(define (problem p) (:domain d)
            (:objects t1 t2 - truck a b c - place)
            (:init (at t1 a) (at t2 a) (= (fuel t1) 7) (= (fuel t2) 4) (= (cost) 0))
            (:goal (and (visited b) (visited c))) (:metric minimize (cost)))";
        let model = from_strings(domain, problem);
        let constraints = CountConstraints::new(&model);
        println!("{constraints}");
        let budget = (constraints.constraints.iter())
            .find(|c| c.generator == "numeric-budget")
            .unwrap();
        // total fuel: 7 + 4, each move consumes 2
        assert_eq!(
            constraints.vars.display(&budget.constraint).to_string(),
            "2*n(move) <= 11"
        );
        assert_eq!(budget.scope, PlanScope::AllPlans);
        assert_eq!(bounds(&model, PlanScope::AllPlans)["move"], "[2, 5]");
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
