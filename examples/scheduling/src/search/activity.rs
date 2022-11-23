use crate::Var;
use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::ref_store::{RefMap, RefVec};
use aries_core::literals::Disjunction;
use aries_core::state::{Domains, Explainer};
use aries_core::*;
use aries_model::extensions::{AssignmentExt, SavedAssignment};
use aries_model::Model;
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
use itertools::Itertools;
use std::collections::HashSet;
use std::iter::FromIterator;

#[derive(Clone)]
pub struct BranchingParams {
    pub prefer_min_value: bool,
    pub allowed_conflicts: u64,
    pub increase_ratio_for_allowed_conflicts: f32,
}

impl Default for BranchingParams {
    fn default() -> Self {
        BranchingParams {
            prefer_min_value: true,
            allowed_conflicts: 100,
            increase_ratio_for_allowed_conflicts: 1.5,
        }
    }
}

/// A branching scheme that first select variables that were recently involved in conflicts.
#[derive(Clone)]
pub struct ActivityBrancher {
    pub params: BranchingParams,
    heap: VarSelect,
    default_assignment: DefaultValues,
    conflicts_at_last_restart: u64,
    num_processed_var: usize,
}

#[derive(Clone, Default)]
struct DefaultValues {
    /// If these default values came from a valid assignment, this is the value of the associated objective
    objective_found: Option<IntCst>,
    /// Default value for variables (some variables might not have one)
    values: RefMap<VarRef, IntCst>,
}

type Lbl = Var;

impl ActivityBrancher {
    pub fn new() -> Self {
        Self::new_with(Default::default())
    }

    pub fn new_with(params: BranchingParams) -> Self {
        ActivityBrancher {
            params,
            heap: VarSelect::new(Default::default()),
            default_assignment: DefaultValues::default(),
            conflicts_at_last_restart: 0,
            num_processed_var: 0,
        }
    }

    pub fn import_vars(&mut self, model: &Model<Lbl>) {
        let mut count = 0;
        // go through the model's variables and declare any newly declared variable
        // TODO: use `advance_by` when it is stabilized. The current usage of `dropping` is very expensive
        //       when compiled without opt-level=3. advance_by should be easier to optimize but dropping will
        //       have to wait to adopt it. [Tracking issue](https://github.com/rust-lang/rust/issues/77404)
        for var in model.state.variables().dropping(self.num_processed_var) {
            if model.state.lb(var) == 0 && model.state.ub(var) == 1 {
                self.heap.declare_decision(var.lt(1), None);
                self.heap.declare_decision(var.gt(0), None);
            }
            count += 1;
        }
        self.num_processed_var += count;
    }

    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    pub fn next_decision(&mut self, stats: &Stats, model: &Model<Lbl>) -> Option<Decision> {
        self.import_vars(model);

        // extract the highest priority variable that is not set yet.
        let next_unset = self.heap.peek(&model.state);

        if let Some(literal) = next_unset {
            if stats.num_conflicts - self.conflicts_at_last_restart >= self.params.allowed_conflicts {
                // we have exceeded the number of allowed conflict, time for a restart
                self.conflicts_at_last_restart = stats.num_conflicts;
                // increase the number of allowed conflicts
                self.params.allowed_conflicts =
                    (self.params.allowed_conflicts as f32 * self.params.increase_ratio_for_allowed_conflicts) as u64;

                Some(Decision::Restart)
            } else {
                Some(Decision::SetLiteral(literal))
            }
        } else {
            // no literal with activity peek the first unbound
            println!("NO ACTIVE");
            model
                .state
                .variables()
                .filter_map(|v| {
                    let dom = model.var_domain(v);
                    if dom.is_bound() {
                        None
                    } else {
                        Some(Decision::SetLiteral(v.leq(dom.lb)))
                    }
                })
                .next()
        }
    }

    pub fn set_default_value(&mut self, var: VarRef, val: IntCst) {
        self.default_assignment.values.insert(var, val);
    }

    // pub fn incumbent_cost(&self) -> Option<IntCst> {
    //     self.default_assignment.objective_found
    // }
    // pub fn set_incumbent_cost(&mut self, cost: IntCst) {
    //     assert!(self.default_assignment.objective_found.iter().all(|&c| c > cost));
    //     self.default_assignment.objective_found = Some(cost);
    // }
}

impl Default for ActivityBrancher {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct DecayParams {
    pub var_inc: f32,
    pub var_decay: f32,
}
impl Default for DecayParams {
    fn default() -> Self {
        DecayParams {
            var_inc: 1_f32,
            var_decay: 0.95_f32,
        }
    }
}

#[derive(Clone, Default)]
pub struct Activities {
    activities: RefVec<VarBound, Vec<(Lit, f32)>>,
}
impl Activities {
    pub fn add(&mut self, lit: Lit, act: f32) {
        let id = lit.affected_bound();
        self.activities.fill_with(id, Vec::new);
        if let Some((_, prev_act)) = self.activities[id].iter_mut().find(|(l, _)| *l == lit) {
            *prev_act += act;
        } else {
            self.activities[id].push((lit, act));
        }
    }

    // pub fn decrease(&mut self, lit: Lit, factor: f32) -> f32 {
    //
    // }

    pub fn remove_events_below(&mut self, cut_off: f32) {
        for k in self.activities.keys() {
            self.activities[k].retain(|&(_, act)| act >= cut_off);
        }
    }

    pub fn activity(&self, lit: Lit, inactive: &impl Fn(Lit) -> bool) -> f32 {
        self.activities[lit.affected_bound()]
            .iter()
            .filter(|&&(l, _)| !inactive(l) && lit.entails(l))
            .map(|(_, act)| act)
            .sum()
    }

    pub fn lits(&self) -> impl Iterator<Item = Lit> + '_ {
        self.activities
            .keys()
            .flat_map(move |vb| self.activities[vb].iter().map(|(l, _)| *l))
    }

    pub fn activities<'a>(&'a self, inactive: impl Fn(Lit) -> bool + 'a) -> impl Iterator<Item = (Lit, f32, f32)> + 'a {
        self.lits()
            // .filter(move |l: &Lit| !inactive(*l))
            .map(move |l| (l, self.activity(l, &inactive), self.activity(!l, &inactive)))
    }

    #[allow(dead_code)]
    pub fn print_activity(&self, lit: Lit, inactive: impl Fn(Lit) -> bool, base_act: f32) {
        for (l, act) in self.activities[lit.affected_bound()]
            .iter()
            .filter(|&&(l, _)| !inactive(l) && lit.entails(l))
            .sorted_by_key(|(l, _)| l)
        {
            println!("{l:?}  --  {}", act / base_act);
        }
    }

    pub fn rescale(&mut self, scale: f32) {
        for k in self.activities.keys() {
            for (_, act) in &mut self.activities[k] {
                *act *= scale;
            }
        }
    }
}

#[derive(Clone)]
pub struct VarSelect {
    params: DecayParams,
    /// One heap for each decision stage.
    activities: Activities,
    saved: DecLvl,
}

impl VarSelect {
    pub fn new(params: DecayParams) -> Self {
        VarSelect {
            params,
            activities: Default::default(),
            saved: DecLvl::ROOT,
        }
    }

    pub fn peek(&mut self, model: &Domains) -> Option<Lit> {
        let mut best: Option<Lit> = None;
        let mut best_act = -f32::INFINITY;

        // println!("");
        for (l, ta, fa) in self.activities.activities(|l| model.value(l).is_some()) {
            if model.value(l).is_none() {
                // let ta = self.activity(l, model);
                // let fa = self.activity(!l, model);
                let a = ta + fa;
                // println!("{l:?} {a}");
                if a > best_act {
                    best_act = a;
                    best = Some(if ta >= fa { l } else { !l });
                }
            }
        }
        // if let Some(best) = best {
        //     println!("\n{best:?}    {}", best_act / self.params.var_inc);
        //     self.activities
        //         .print_activity(best, |l| model.value(l).is_some(), self.params.var_inc);
        // }
        best
    }

    pub fn bump_activity(&mut self, l: Lit, weight: f32) {
        let act = self.params.var_inc * weight;
        self.activities.add(l, act);
    }
    // pub fn activity(&mut self, l: Lit, dom: &Domains) -> f32 {
    //     self.activities.activity(l, |l| dom.value(l).is_some())
    // }

    /// Declares a new variable. The variable is NOT added to the queue.
    /// The stage parameter defines at which stage of the search the variable will be selected.
    /// Variables with the lowest stage are considered first.
    pub fn declare_decision(&mut self, lit: Lit, initial_activity: Option<f32>) {
        self.bump_activity(lit, initial_activity.unwrap_or(1.0));
    }

    pub fn decay_activities(&mut self) {
        self.params.var_inc /= self.params.var_decay;
        if self.params.var_inc >= 1e30_f32 {
            self.var_rescale_activity()
        }
        self.activities.remove_events_below(self.params.var_inc / 1000000_f32);
    }

    fn var_rescale_activity(&mut self) {
        let scale = 1e-30_f32;
        self.activities.rescale(scale);
        self.params.var_inc *= scale;
    }
}
impl Backtrack for VarSelect {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}

impl Backtrack for ActivityBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.heap.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.heap.num_saved()
    }

    fn restore_last(&mut self) {
        self.heap.restore_last()
    }
}

impl SearchControl<Var> for ActivityBrancher {
    fn next_decision(&mut self, stats: &Stats, model: &Model<Var>) -> Option<Decision> {
        self.next_decision(stats, model)
    }

    fn import_vars(&mut self, model: &Model<Var>) {
        self.import_vars(model)
    }

    fn new_assignment_found(&mut self, objective: IntCst, assignment: std::sync::Arc<SavedAssignment>) {
        // if we are in LNS mode and the given solution is better than the previous one,
        // set the default value of all variables to the one they have in the solution.
        let is_improvement = self
            .default_assignment
            .objective_found
            .map(|prev| objective < prev)
            .unwrap_or(true);
        if is_improvement {
            self.default_assignment.objective_found = Some(objective);
            for (var, val) in assignment.bound_variables() {
                self.set_default_value(var, val);
            }
        }
    }

    fn conflict(&mut self, clause: &Disjunction, model: &Model<Var>, _explainer: &mut dyn Explainer) {
        // Provides a vector with all literals associated with their decision level, sorted by decision level
        let with_lvl = |lits: &[Lit]| {
            lits.iter()
                .map(|&l| {
                    if model.entails(!l) {
                        (Some(model.state.entailing_level(!l)), l)
                    } else {
                        (None, l)
                    }
                })
                .sorted()
                .collect::<Vec<_>>()
        };
        let lits = with_lvl(clause.literals());
        let conflict_levels: HashSet<DecLvl> = HashSet::from_iter(lits.iter().filter_map(|(lvl, _)| *lvl));
        // literals block distance of "Predicting Learnt Clause Quality in Modern SAT solvers"
        let lbd = conflict_levels.len();
        // println!("LBD: {lbd} / {}", clause.len());

        // bump activity of all variables of the clause
        self.heap.decay_activities();
        let weight = 1.0 / lbd as f32;
        // let weight = 1.0;
        for b in clause.literals() {
            self.heap.bump_activity(!*b, weight);
        }

        // let lits = model.state.decisions_only(clause.literals().to_vec(), _explainer);
        //
        // let lits = with_lvl(lits.literals());
        // let conflict_levels: HashSet<DecLvl> = HashSet::from_iter(lits.iter().filter_map(|(lvl, _)| *lvl));
        //
        // println!();
        // for (lvl, l) in model.state.decisions() {
        //     if conflict_levels.contains(&lvl) {
        //         println!(" ->  {lvl:?} {l:?}",)
        //     } else {
        //         println!("     {lvl:?} {l:?}",)
        //     }
        // }

        // println!();
        // for (lvl, l) in with_lvl(clause.literals()) {
        //     if let Some(lvl) = lvl {
        //         println!(":     {lvl:?} {l:?}",)
        //     } else {
        //         println!(":     ->  {l:?}",)
        //     }
        // }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Lbl> + Send> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aries_core::state::Cause;
    #[test]
    fn test_activities() {
        let v1 = VarRef::from_u32(1);

        let mut acts = Activities::default();
        acts.add(v1.leq(3), 1.0);
        acts.add(v1.leq(5), 1.0);
        acts.add(v1.gt(4), 1.0);
        acts.add(v1.gt(5), 1.0);
        acts.add(v1.gt(5), 1.0);
        acts.add(v1.gt(6), 1.0);
        acts.add(v1.gt(5), 2.0);
        let act = |l| acts.activity(l, &|_| false);
        assert_eq!(act(v1.leq(5)), 1.0);
        assert_eq!(act(v1.leq(3)), 2.0);
        assert_eq!(act(v1.gt(4)), 1.0);
        assert_eq!(act(v1.gt(5)), 5.0);
        assert_eq!(act(v1.gt(6)), 6.0);
        // simulate the fact the (v1 > 5) and (v1 <= 5) is already decided is already decided
        let dec = v1.gt(5);
        let act = |l| acts.activity(l, &|l| dec.entails(l) || (!dec).entails(l));
        assert_eq!(act(v1.leq(5)), 0.0);
        assert_eq!(act(v1.leq(3)), 1.0);
        assert_eq!(act(v1.gt(4)), 0.0);
        assert_eq!(act(v1.gt(5)), 0.0);
        assert_eq!(act(v1.gt(6)), 1.0);
        acts.remove_events_below(1.5);
        let act = |l| acts.activity(l, &|_| false);
        assert_eq!(act(v1.leq(5)), 0.0);
        assert_eq!(act(v1.leq(3)), 0.0);
        assert_eq!(act(v1.gt(4)), 0.0);
        assert_eq!(act(v1.gt(5)), 4.0); // the total activity is kept
        assert_eq!(act(v1.gt(6)), 4.0);
    }

    #[test]
    fn test_var_select() {
        let mut model: Model<&str> = Model::new();

        let a = model.new_ivar(0, 10, "a");
        let b = model.new_ivar(0, 10, "b");
        let c = model.new_ivar(0, 10, "c");

        let mut chooser = VarSelect::new(DecayParams::default());
        let clauses = [[a.leq(5), b.leq(5)], [a.leq(4), c.leq(4)], [a.gt(7), b.leq(5)]];

        for cl in &clauses {
            for &l in cl {
                chooser.bump_activity(l, 1.0);
            }
        }
        let mut peek_set = |expected: Lit| {
            assert_eq!(chooser.peek(&model.state), Some(expected));
            assert_eq!(model.state.set(expected, Cause::Decision), Ok(true));
        };
        peek_set(a.leq(4));
        peek_set(b.leq(5));
        peek_set(c.leq(4));
        assert_eq!(chooser.peek(&model.state), None);
    }
}
