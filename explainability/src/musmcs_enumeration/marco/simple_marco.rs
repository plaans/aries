use aries::backtrack::Backtrack;
use aries::core::{Lit, INT_CST_MAX};
use aries::model::extensions::AssignmentExt;
use aries::model::lang::expr::or;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::IAtom;
use aries::model::{Label, Model};
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::reif::Reifiable;
use aries::solver::Solver;
use itertools::Itertools;

use crate::musmcs_enumeration::marco::{MapSolver, Marco, SubsetSolver};
use crate::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

struct SimpleMapSolver {
    s: Solver<u8>,
    literals: BTreeSet<Lit>,
    to_max: IAtom,
    lits_translate_in: BTreeMap<Lit, Lit>, // Maps the soft constraint reification literals of the subset solver to those of the map solver (self).
    lits_translate_out: BTreeMap<Lit, Lit>, // Maps the soft constraint reification literals of the map solver (self) to those of the subset solver.
}

impl SimpleMapSolver {
    fn new(literals: impl IntoIterator<Item = Lit>) -> Self {
        let mut model = Model::new();
        let mut lits_translate_in = BTreeMap::<Lit, Lit>::new();
        let mut lits_translate_out = BTreeMap::<Lit, Lit>::new();

        for literal in literals {
            let lit = model.state.new_var(0, 1).geq(1);
            lits_translate_in.insert(literal, lit);
            lits_translate_out.insert(lit, literal);
        }

        let literals: BTreeSet<Lit> = lits_translate_out.keys().cloned().collect();

        let to_max = IAtom::from(model.state.new_var(0, INT_CST_MAX));
        let literals_sum = LinearSum::of(literals.iter().map(|&l| IAtom::from(l.variable())).collect_vec());
        model.enforce(literals_sum.clone().leq(to_max), []);
        model.enforce(literals_sum.geq(to_max), []);

        model.enforce(or(literals.iter().cloned().collect_vec()), []);

        SimpleMapSolver {
            s: Solver::<u8>::new(model),
            literals,
            to_max,
            lits_translate_in,
            lits_translate_out,
        }
    }
}

impl MapSolver for SimpleMapSolver {
    fn find_unexplored_seed(&mut self) -> Option<BTreeSet<Lit>> {
        match self.s.maximize(self.to_max).unwrap() {
            Some((_, best_assignment)) => {
                let seed = Some(
                    self.literals
                        .iter()
                        .filter(|&&l| best_assignment.entails(l))
                        .map(|&l| self.lits_translate_out[&l])
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
        let complement = self
            .literals
            .difference(&sat_subset.iter().map(|&l| self.lits_translate_in[&l]).collect())
            .cloned()
            .collect_vec();
        self.s.enforce(or(complement), []);
    }

    fn block_up(&mut self, unsat_subset: &BTreeSet<Lit>) {
        let neg = unsat_subset.iter().map(|&l| !self.lits_translate_in[&l]).collect_vec();
        self.s.enforce(or(neg), []);
    }

    // fn get_internal_solver(&mut self) -> &mut Solver<u8> {
    //     &mut self.s
    // }
}

struct SimpleSubsetSolver<Lbl: Label> {
    s: Solver<Lbl>,
    soft_constrs_reif_lits: Arc<BTreeSet<Lit>>,
    cached_unsat_core: BTreeSet<Lit>,
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
        }
    }
}

impl<Lbl: Label> SubsetSolver<Lbl> for SimpleSubsetSolver<Lbl> {
    fn check_seed_sat(&mut self, seed: &BTreeSet<Lit>) -> bool {
        // FIXME warm-start / solution hints optimization should go here... right ?
        let res = self
            .s
            .solve_with_assumptions(seed.iter().cloned())
            .expect("Solver interrupted...");
        self.s.reset();
        if let Err(unsat_core) = res {
            self.cached_unsat_core.clear();
            self.cached_unsat_core.extend(unsat_core.literals());
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
        let mcs: BTreeSet<Lit> = self.soft_constrs_reif_lits.difference(&mss).cloned().collect();
        (mss, Some(mcs))
    }

    fn shrink(&mut self, seed: &BTreeSet<Lit>) -> BTreeSet<Lit> {
        let mut mus: BTreeSet<Lit> = seed.clone();
        for &lit in seed {
            if !mus.contains(&lit) {
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

    // fn get_internal_solver(&mut self) -> &mut Solver<Lbl> {
    //     &mut self.s
    // }
}

pub struct SimpleMarco<Lbl: Label> {
    // config: MusMcsEnumerationConfig,
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

        let map_solver = SimpleMapSolver::new(soft_constrs_reif_lits.iter().cloned());
        let subset_solver = SimpleSubsetSolver::<Lbl>::new(model, soft_constrs_reif_lits.clone());

        Self {
            // config,
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
            // from Ignace Bleukx's implementation:
            // find more MCSes, **disjoint** from this one, similar to "optimal_mus" in mus.py
            // can only be done when MCSes do not have to be returned as there is no guarantee
            // the MCSes encountered during enumeration are "new" MCSes
            let mut sat_subset_lits: BTreeSet<Lit> = self.soft_constrs_reif_lits
                .iter()
                .filter(|&&l| self.subset_solver.s.model.entails(!l))
                .cloned()
                .collect();
            self.map_solver
                .s
                .enforce(or(
                        sat_subset_lits
                        .iter()
                        .map(|&l| self.map_solver.lits_translate_in[&l])
                        .collect_vec()
                    ), []);
            while self.subset_solver.check_seed_sat(&sat_subset_lits) {
                let s2: BTreeSet<Lit> = self
                    .soft_constrs_reif_lits
                    .iter()
                    .filter(|&&l| self.subset_solver.s.model.entails(l))
                    .cloned()
                    .collect();
                let new_mcs = self.soft_constrs_reif_lits.difference(&s2).cloned();
                sat_subset_lits.extend(new_mcs.clone());
                self.map_solver.s
                .enforce(or(
                    new_mcs
                    .map(|l| self.map_solver.lits_translate_in[&l])
                    .collect_vec()
                ), []);

            }
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
