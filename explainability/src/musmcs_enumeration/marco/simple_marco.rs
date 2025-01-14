use aries::backtrack::Backtrack;
use aries::core::{Lit, SignedVar, VarRef, INT_CST_MAX, INT_CST_MIN};
use aries::model::extensions::{AssignmentExt, SavedAssignment};
use aries::model::lang::expr::or;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::IAtom;
use aries::model::{Label, Model};
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::reif::Reifiable;
use aries::solver::search::combinators::CombinatorExt;
use aries::solver::search::lexical::Lexical;
use aries::solver::search::SearchControl;
use aries::solver::Solver;
use itertools::Itertools;

use crate::musmcs_enumeration::marco::{MapSolver, Marco, SubsetSolver};
use crate::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

struct SimpleMapSolver {
    s: Solver<u8>,
    /// Maps signed(!) variables of the literals representing the soft constraints in the subset solver to (not signed!) variables in the map solver (self).
    /// The "opposite" of `vars_translate_out`.
    /// 
    /// See documentation for `vars_translate_out` for more information.
    vars_translate_in: BTreeMap<SignedVar, VarRef>,
    /// Maps (not signed!) variables of the literals representing the soft constraints in the map solver (self) to signed(!) variables in the subset solver.
    /// The "opposite" of `vars_translate_in`.
    /// 
    /// The reason why we don't do signed / signed or unsigned / unsigned mapping, is because that could result
    /// in a bug where some seeds would not be discovered. For example, the seed {[a <= 2], [a >= 3]}
    /// would never be discovered in `find_unexplored_seed`, because it is already trivially unsatisfiable.
    /// On the other hand, if we had two variables a and a', this bug wouldn't happen.
    /// 
    /// It should be noted that we could, theoretically, use the fact that a seed like {[a <= 2], [a >= 3]}
    /// would never be discovered to our advantadge. Indeed, being trivially unsatisfiable,
    /// we could inform the subset solver directly of a trivially unsatisfiable seed,
    /// without even needing to do a solve call to discover it.
    /// However, the implementation for this could be messy / complicated, and would stray
    /// too far from the pseudo-code / "standard" of the MARCO algorithm.
    vars_translate_out: BTreeMap<VarRef, SignedVar>,
    /// These literals represent soft constraints in the map solver (self).
    /// Their variables are the same as the keys of `vars_translate_out`.
    literals: BTreeSet<Lit>,
    solve_fn: Box<dyn FnMut(&mut Solver<u8>) -> Option<Arc<SavedAssignment>>>,
}

fn signed_var_of_same_sign(var: VarRef, svar: SignedVar) -> SignedVar {
    if svar.is_plus() {
        SignedVar::plus(var)
    } else if svar.is_minus() {
        SignedVar::minus(var)
    } else {
        panic!()
    }
}

impl SimpleMapSolver {
    fn new(literals: impl IntoIterator<Item = Lit>) -> Self {
        let mut model = Model::new();

        let mut vars_translate_in = BTreeMap::<SignedVar, VarRef>::new();
        let mut vars_translate_out = BTreeMap::<VarRef, SignedVar>::new();

        // Literals representing the soft constraints
        let literals = literals
            .into_iter()
            .map(|lit| {
                  // WARNING!! Here, do NOT use `or_insert` instead of `or_insert_with` !!
                  // As this is going to execute `model.state.new_var(..)` (i.e. create a new variable)
                  // EVEN if the entry is non-empty ...! 
                let v: VarRef = *vars_translate_in
                    .entry(lit.svar())
                    .or_insert_with(|| model.state.new_var(INT_CST_MIN, INT_CST_MAX));
                vars_translate_out.entry(v).or_insert(lit.svar());
                Lit::new(signed_var_of_same_sign(v, lit.svar()), lit.ub_value())
            })
            .collect::<BTreeSet<Lit>>();

        // TODO do preferred values instead of maximize.
        // TODO option to use minimize. (for "low bias")
        // TODO cardinality-like constraints ? (another of possible liffiton optimizations)

        let mut s = Solver::<u8>::new(model);

        // FIXME usage of strings is dirty / temporary
        let solving_mode = "PREFERRED_VALUES_HIGH";

        // Approaches for finding / solving for unexplored seeds.
        //
        // "High" bias approaches are more likely to discover UNSAT seeds early (since they will be larger), thus favoring finding MUSes early.
        // "Low" bias approaches are more likely to discover SAT seeds early (since they will be smaller), thus favoring finding MCSes early.
        //
        // This can be done by maximizing (high) or minimizing (low) the cardinality / sum of literals' indicator variables.
        // Another functionally equivalent approach - expected to be more performant - is to inform the solver
        // of our default / preferred values for the literals' indicator variables (0 - low, 1 - high).
        let solve_fn: Box<dyn FnMut(&mut Solver<u8>) -> Option<Arc<SavedAssignment>>> = match solving_mode {
            // Optimize the sum of the literals' indicator variables. (Maximize for high bias)
            "OPTIMIZE_HIGH" => {
                let sum = LinearSum::of(
                    literals
                        .iter()
                        .map(|&l| {
                            let v = s.model.state.new_var(0, 1);
                            s.model.bind(l, v.geq(1));
                        
                            IAtom::from(v)
                        })
                        .collect_vec(),
                );
                let obj = IAtom::from(s.model.state.new_var(0, INT_CST_MAX));
                s.model.enforce(sum.geq(obj), []);

                Box::new(
                    move |_s: &mut Solver<u8>| {
                        (*_s).maximize(obj).expect("Solver interrupted").map(|(_, doms)| doms)
                    }
                )
            }
            // Optimize the sum of the literals' indicator variables. (Minimize for low bias)
            "OPTIMIZE_LOW" => {
                let sum = LinearSum::of(
                    literals
                        .iter()
                        .map(|&l| {
                            let v = s.model.state.new_var(0, 1);
                            s.model.bind(l, v.geq(1));
                        
                            IAtom::from(v)
                        })
                        .collect_vec(),
                );
                let obj = IAtom::from(s.model.state.new_var(0, INT_CST_MAX));
                s.model.enforce(sum.leq(obj), []);

                Box::new(
                    move |_s: &mut Solver<u8>| {
                        (*_s).minimize(obj).expect("Solver interrupted").map(|(_, doms)| doms)
                    }
                )
            }
            // Ask the solver to, if possible, set the literals to true (high bias).
            "PREFERRED_VALUES_HIGH" => {
                let brancher = Lexical::with_vars(
                    literals.iter().map(|&l| {
                        let v = s.model.state.new_var(0, 1);
                        s.model.bind(l, v.geq(1));
                        v
                    }),
                    aries::solver::search::lexical::PreferredValue::Max,
                )
                .clone_to_box()
                .and_then(s.brancher.clone_to_box());
                
                s.set_brancher_boxed(brancher);

                Box::new(move |_s: &mut Solver<u8>| (*_s).solve().expect("Solver interrupted"))
            }
            // Ask the solver to, if possible, set the literals to false (low bias).
            "PREFERRED_VALUES_LOW" => {
                let brancher = Lexical::with_vars(
                    literals.iter().map(|&l| {
                        let v = s.model.state.new_var(0, 1);
                        s.model.bind(l, v.geq(1));
                        v
                    }),
                    aries::solver::search::lexical::PreferredValue::Min,
                )
                .clone_to_box()
                .and_then(s.brancher.clone_to_box());
                
                s.set_brancher_boxed(brancher);
            
                Box::new(move |_s: &mut Solver<u8>| (*_s).solve().expect("Solver interrupted"))
            }
            // Unoptimised approach, any solutions.
            "NOTHING" => {
                Box::new(move |_s: &mut Solver<u8>| (*_s).solve().expect("Solver interrupted"))
            }
            _ => panic!()
        };
        // s.model.enforce(or(literals.iter().copied().collect_vec()), []); // Could adding this be any useful for performance ?

        SimpleMapSolver {
            s,
            vars_translate_in,
            vars_translate_out,
            literals,
            solve_fn,
        }
    }

    /// Translates a soft constraint reification literal from its subset solver representation to its map solver representation.
    fn translate_lit_in(&self, literal: Lit) -> Lit {
        Lit::new(
            signed_var_of_same_sign(self.vars_translate_in[&literal.svar()], literal.svar()),
            literal.ub_value(),
        )
    }

    /// Translates a soft constraint reification literal from its map solver representation to its subset solver representation.
    fn translate_lit_out(&self, literal: Lit) -> Lit {
        Lit::new(
            self.vars_translate_out[&literal.variable()],
            literal.ub_value(),
        )
    }
}

impl MapSolver for SimpleMapSolver {
    fn find_unexplored_seed(&mut self) -> Option<BTreeSet<Lit>> {
        match (self.solve_fn)(&mut self.s) {
            Some(best_assignment) => {
                let seed = Some(
                    self.literals
                        .iter()
                        .filter(|&&l| best_assignment.entails(l))
                        .map(|&l| self.translate_lit_out(l))
                        .collect(),
                );
                self.s.reset();
                seed
            }
            None => {
                self.s.reset();
                None
            }
        }
    }

    fn block_down(&mut self, sat_subset: &BTreeSet<Lit>) {
        let translated_sat_subset = sat_subset.iter().map(|&l| self.translate_lit_in(l)).collect();
        let translated_sat_subset_complement = self.literals.difference(&translated_sat_subset).copied().collect_vec();
        self.s.enforce(or(translated_sat_subset_complement), []);
    }

    fn block_up(&mut self, unsat_subset: &BTreeSet<Lit>) {
        let translated_unsat_subset_negs = unsat_subset.iter().map(|&l| !self.translate_lit_in(l)).collect_vec();
        self.s.enforce(or(translated_unsat_subset_negs), []);
    }
}

struct SimpleSubsetSolver<Lbl: Label> {
    s: Solver<Lbl>,
    soft_constrs_reif_lits: Arc<BTreeSet<Lit>>,
    /// The latest unsat core computed in `check_seed_sat` (in the `false` return case).
    cached_unsat_core: BTreeSet<Lit>,
    /// Used for an optimization. Set of (soft constraint reification) literals
    /// that have been found to constitute a singleton MCS, i.e. belonging to all MUSes.
    necessarily_in_all_muses: BTreeSet<Lit>,
}

impl<Lbl: Label> SimpleSubsetSolver<Lbl> {
    fn new(model: Model<Lbl>, soft_constrs_reif_lits: Arc<BTreeSet<Lit>>) -> Self {
        let stn_config = StnConfig {
            theory_propagation: TheoryPropagationLevel::Full,
            ..Default::default()
        };
        let mut s = Solver::<Lbl>::new(model);
        s.reasoners.diff.config = stn_config;

        SimpleSubsetSolver::<Lbl> {
            s,
            soft_constrs_reif_lits,
            cached_unsat_core: BTreeSet::new(),
            necessarily_in_all_muses: BTreeSet::new(),
        }
    }
}

impl<Lbl: Label> SubsetSolver<Lbl> for SimpleSubsetSolver<Lbl> {
    fn check_seed_sat(&mut self, seed: &BTreeSet<Lit>) -> bool {
        // FIXME warm-start / solution hints optimization should go here... right ?
        let res = self
            .s
            .solve_with_assumptions(seed.iter().copied())
            .expect("Solver interrupted...");
        self.s.reset();

        if let Err(unsat_core) = res {
            self.cached_unsat_core = unsat_core
                .literals()
                .into_iter()
                .chain(&self.necessarily_in_all_muses)
                .copied()
                .collect();
            false
        } else {
            true
        }
    }

    fn grow(&mut self, seed: &BTreeSet<Lit>) -> (BTreeSet<Lit>, Option<BTreeSet<Lit>>) {
        let mut mss = seed.clone();
        for &lit in self.soft_constrs_reif_lits.clone().difference(seed) {
            mss.insert(lit);
            if !self.check_seed_sat(&mss) {
                mss.remove(&lit);
            }
        }
        let mcs: BTreeSet<Lit> = self.soft_constrs_reif_lits.difference(&mss).copied().collect();

        // If the found correction set only has 1 element,
        // then that element is added to those that are known to be in all unsatisfiable sets.
        if mcs.len() == 1 {
            self.necessarily_in_all_muses.insert(mcs.first().unwrap().clone());
        }
        (mss, Some(mcs))
    }

    fn shrink(&mut self, seed: &BTreeSet<Lit>) -> BTreeSet<Lit> {
        let mut mus: BTreeSet<Lit> = seed.clone();
        for &lit in seed {
            if !mus.contains(&lit) {
                continue;
            }
            // Optimization: if the literal has been determined to belong to all muses,
            // no need to check if, without it, the set would be satisfiable (because it obviously would be). 
            if self.necessarily_in_all_muses.contains(&lit) {
                continue;
            }
            mus.remove(&lit);
            if !self.check_seed_sat(&mus) {
                mus = self.cached_unsat_core.clone();
            } else {
                debug_assert!(!mus.contains(&lit));
                mus.insert(lit);
            }
        }
        mus
    }
}

pub struct SimpleMarco<Lbl: Label> {
    cached_result: MusMcsEnumerationResult,
    seed: BTreeSet<Lit>,
    map_solver: SimpleMapSolver,
    subset_solver: SimpleSubsetSolver<Lbl>,
    soft_constrs_reif_lits: Arc<BTreeSet<Lit>>,
}

impl<Lbl: Label> Marco<Lbl> for SimpleMarco<Lbl> {
    fn new_with_soft_constrs_reif_lits(
        model: Model<Lbl>,
        soft_constrs_reif_lits: impl IntoIterator<Item = Lit>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let cached_result = MusMcsEnumerationResult {
            muses: if config.return_muses {
                Some(Vec::<BTreeSet<Lit>>::new())
            } else {
                None
            },
            mcses: if config.return_mcses {
                Some(Vec::<BTreeSet<Lit>>::new())
            } else {
                None
            },
        };
        debug_assert_eq!(cached_result.muses.is_some(), config.return_muses);
        debug_assert_eq!(cached_result.mcses.is_some(), config.return_mcses);

        let soft_constrs_reif_lits = Arc::new(BTreeSet::from_iter(soft_constrs_reif_lits));

        let map_solver = SimpleMapSolver::new(soft_constrs_reif_lits.iter().copied());
        let subset_solver = SimpleSubsetSolver::<Lbl>::new(model, soft_constrs_reif_lits.clone());

        Self {
            cached_result,
            seed: BTreeSet::new(),
            map_solver,
            subset_solver,
            soft_constrs_reif_lits,
        }
    }

    fn new_with_soft_constrs<Expr: Reifiable<Lbl>>(
        model: Model<Lbl>,
        soft_constrs: impl IntoIterator<Item = Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let mut model = model.clone();
        let soft_constrs_reif_lits = soft_constrs.into_iter().map(|expr| model.reify(expr)).collect_vec();

        Self::new_with_soft_constrs_reif_lits(model, soft_constrs_reif_lits, config)
    }

    fn reset_result(&mut self) {
        if let Some(ref mut muses) = self.cached_result.muses {
            muses.clear();
        }
        if let Some(ref mut mcses) = self.cached_result.mcses {
            mcses.clear();
        }
    }

    fn clone_result(&self) -> MusMcsEnumerationResult {
        self.cached_result.clone()
    }

    fn get_expr_reif_lit<Expr: Reifiable<Lbl>>(&mut self, soft_constr: Expr) -> Option<Lit> {
        self.subset_solver.s.model.check_reified(soft_constr)
    }

    fn find_unexplored_seed(&mut self) -> bool {
        match self.map_solver.find_unexplored_seed() {
            Some(next_seed) => {
                self.seed = next_seed;
                true
            }
            None => false,
        }
    }

    fn check_seed_sat(&mut self) -> bool {
        self.subset_solver.check_seed_sat(&self.seed)
    }

    fn do_case_seed_sat(&mut self) {
        if let Some(ref mut mcses) = self.cached_result.mcses {
            let (mss, mcs) = self.subset_solver.grow(&self.seed);
            self.map_solver.block_down(&mss);
            mcses.push(mcs.unwrap());
        } else {
            self.case_seed_sat_only_muses_optimization();
        }
    }

    fn do_case_seed_unsat(&mut self) {
        let mus = self.subset_solver.shrink(&self.seed);
        self.map_solver.block_up(&mus);
        if let Some(ref mut muses) = self.cached_result.muses {
            muses.push(mus);
        }
    }
}

impl<Lbl: Label> SimpleMarco<Lbl> {
    fn case_seed_sat_only_muses_optimization(&mut self) {
        // Optimization inspired by the implementation of Ignace Bleukx (in python).
        //
        // If we are not going to return MCSes, we can try to greedily search for
        // more correction subsets, disjoint from this one (the seed).
        //
        // This can only be done when we only intend to return MUSes, not MCSes,
        // because the correction sets we greedily discover with this optimization
        // have no guarantee of being unique / not having been already discovered.

        let mut sat_subset = self.seed.clone();
        self.map_solver.block_down(&sat_subset);

        // Another optimization (*):
        // If the found correction set only has 1 element,
        // then that element is added to those that are known to be in all unsatisfiable sets.
        if let Some(&lit) = self
            .soft_constrs_reif_lits
            .difference(&sat_subset)
            .take(2)
            .fold(None, |opt, item| opt.xor(Some(item)))
        {
            self.subset_solver.necessarily_in_all_muses.insert(lit);
        }

        // Grow the sat subset as much as possible (i.e. until unsatisfiability
        // by extending it with each correction set discovered.
        while self
            .subset_solver
            .s
            .solve_with_assumptions(sat_subset.iter().copied())
            .expect("Solver interrupted...")
            .is_ok()
        {
            let new_sat_subset: BTreeSet<Lit> = self
                .soft_constrs_reif_lits
                .iter()
                .filter(|&&l| self.subset_solver.s.model.entails(l))
                .copied()
                .collect();
            self.map_solver.block_down(&new_sat_subset);

            let new_corr_subset = self.soft_constrs_reif_lits.difference(&new_sat_subset).into_iter();
            sat_subset.extend(new_corr_subset.clone());

            // Same optimization as (*) above
            if let Some(&lit) = new_corr_subset.take(2).fold(None, |opt, item| opt.xor(Some(item))) {
                self.subset_solver.necessarily_in_all_muses.insert(lit);
            }

            self.subset_solver.s.reset();
        }
        self.subset_solver.s.reset();
    }
}
