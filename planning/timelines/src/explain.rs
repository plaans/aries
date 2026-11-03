use std::collections::{BTreeMap, BTreeSet};

use aries_solver::prelude::*;
use aries_solver::reasoners::lp::LpOptions;
use aries_solver::{
    backtrack::Backtrack,
    solver::{Solver, musmcs::MusMcs},
};
use itertools::Itertools;

use crate::lprelax::wrappers::LpRelaxIncr;
use crate::{ConstraintID, IntExp, Sched};

/// Stops the search at its first decision, i.e. once the root has been propagated
/// (to evaluate what the propagation at the root alone can prove).
static ARIES_STOP_AT_ROOT: aries_env_param::EnvParam<bool> =
    aries_env_param::EnvParam::new("ARIES_STOP_AT_ROOT", "false");

pub struct ExplainableSolver<T> {
    solver: Solver<crate::Sym>,
    enablers: BTreeMap<Lit, T>,
}

impl<T: Ord + Clone> ExplainableSolver<T> {
    /// Creates a new explainability oriented solver where constraints are partitioned into:
    ///
    ///  - background constraints (strong), for which the projection returns `None`
    ///  - foreground constraints (soft), enabled by assumptions. Two foregrounds constraints with the
    ///    same projection will be enabled together by the same assumption.
    pub fn new(sched: &Sched, project: impl Fn(ConstraintID) -> Option<T>) -> Self {
        let mut encoding = sched.clone().encoder();

        let mut assumptions_map = BTreeMap::new();
        let mut trigger = BTreeMap::new();

        for (cid, c) in sched.constraints.iter().enumerate() {
            tracing::debug!("Adding constraint: {c:?}");
            if let Some(tag) = project(cid) {
                let l = if let Some(l) = trigger.get(&tag) {
                    *l
                } else {
                    let l = encoding.new_optional_bool_var(Lit::TRUE); // could we use the conjunctive scope directly?
                    assumptions_map.insert(l, tag.clone());
                    trigger.insert(tag, l);
                    l
                };
                c.opt_enforce_if(l, &mut encoding);
            } else {
                c.enforce(&mut encoding);
            }
        }
        let mut solver = match crate::lprelax::ARIES_LPRELAX_USE.get_ref().as_str() {
            "incr" => {
                let model = encoding.store.clone();
                let reasoner = LpRelaxIncr::new(encoding, LpOptions::default());
                Solver::with_extra_reasoners(model, vec![Box::new(reasoner)])
            }
            _ => Solver::new(encoding.store),
        };

        // enable stronger propagation than default in difference logic solver.
        // this is useful in planning models where bounds are not sufficient to reason on precedence between tasks
        solver.reasoners.diff().config.theory_propagation = aries_solver::reasoners::stn::TheoryPropagationLevel::Full;

        Self {
            solver,
            enablers: assumptions_map,
        }
    }

    pub fn get(&self) -> &Solver<crate::Sym> {
        &self.solver
    }

    /// Check if the model is satisfiable with all assumptions, and returns a solution if it is.
    pub fn check_satisfiability(&mut self) -> Option<Solution> {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let res = self
            .solver
            .solve_with_assumptions(&assumptions, aries_solver::solver::SearchLimit::None)
            .unwrap()
            .ok();
        self.solver.print_stats();
        self.solver.reset(); // TODO: this should not be needed
        res
    }

    /// Find an optimal solution with all assumptions enforced.
    /// The method accepts an additional set of assumptions that will be enforced in all solutions.
    pub fn find_optimal(
        &mut self,
        obj: LinTerm,
        on_new_solution: impl FnMut(&Solution),
        under_assumptions: impl Into<Vec<Lit>>,
    ) -> Option<Solution> {
        let mut assumptions = under_assumptions.into();
        // add assumptions for detecting unsatisfiable constraints
        for &enabler in self.enablers.keys() {
            assumptions.push(enabler);
        }
        let limit = if ARIES_STOP_AT_ROOT.get() {
            // (the limit is checked before each propagation: allowing one decision lets the root be propagated)
            aries_solver::solver::SearchLimit::NumDecisions(1)
        } else {
            aries_solver::solver::SearchLimit::None
        };
        let start = std::time::Instant::now();
        let res = (self.solver).minimize_with_assumptions(obj, &assumptions, limit, on_new_solution);
        self.solver.print_stats();
        if ARIES_STOP_AT_ROOT.get() {
            // (the solver's stats don't count the time of a search stopped by its limit)
            println!("Root time: {} s", start.elapsed().as_secs_f64());
        }
        let Ok(res) = res else {
            println!("Stopped at the root (no conclusion)");
            std::process::exit(0);
        };
        self.solver.reset(); // TODO: this should not be needed
        res.ok()
    }

    /// Returns an iterator over all MUS and MCS in the model.
    pub fn explain_unsat<'x>(&'x mut self) -> impl Iterator<Item = MusMcs<T>> + 'x {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let projection = |l: &Lit| self.enablers.get(l).cloned();
        self.solver
            .mus_and_mcs_enumerator(&assumptions)
            .map(move |mm| mm.project(projection))
    }

    /// Returns an iterator over all MUS (Minimal Unsatisfiable Subsets) in the model.
    pub fn muses(&mut self) -> impl Iterator<Item = BTreeSet<T>> + '_ {
        self.explain_unsat().filter_map(|mus_mcs| match mus_mcs {
            MusMcs::Mus(mus) => Some(mus),
            MusMcs::Mcs(_) => None,
        })
    }

    /// Returns the smallest MCS over all assumptions
    pub fn find_smallest_mcs(&mut self) -> Option<BTreeSet<T>> {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let num_assumptions = assumptions.len() as IntCst;

        // create a variable that requires a maximal number of assumptions to hold
        // ` |{ass | holds(ass), for ass in assumptions}| <= bound`
        let max_relaxed_assumptions = self.solver.model.new_ivar(0, assumptions.len() as IntCst, "objective");
        let num_relaxed_assumptions = assumptions
            .iter()
            .fold(IntExp::cst(num_assumptions), |sum, l| sum - l.variable());
        self.solver
            .enforce_scoped(num_relaxed_assumptions.leq(max_relaxed_assumptions), []);

        for allowed_relaxations in 0..num_assumptions {
            println!("Current lower bound: {}", allowed_relaxations);
            let result = self
                .solver
                .solve_with_assumptions(
                    &[max_relaxed_assumptions.leq(allowed_relaxations)],
                    aries_solver::solver::SearchLimit::None,
                )
                .unwrap();
            match result {
                Ok(sol) => {
                    self.solver.print_stats();
                    println!("OPTIMAL: {allowed_relaxations} / {} ", num_assumptions);
                    return Some(
                        assumptions
                            .into_iter()
                            .filter(|l| sol.entails(!*l))
                            .map(|l| self.enablers[&l].clone())
                            .collect(),
                    );
                }
                Err(core) if core.literals().is_empty() => {
                    // UNSAT with no assumptions to relax
                    return None;
                }
                Err(_core) => {
                    // assumption makes problem unsat, relax in next round
                }
            };
            self.solver.reset();
        }
        None
    }
}
