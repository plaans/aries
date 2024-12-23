use crate::musmcs_enumeration::SoftConstraintsReifications;
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

use super::{MapSolver, Marco, MusMcsEnumerationConfig, MusMcsEnumerationResult, SubsetSolver};
use std::collections::BTreeSet;
use std::sync::Arc;

fn create_solver<Lbl: Label>(model: Model<Lbl>) -> Solver<Lbl> {
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };
    let mut solver = Solver::<Lbl>::new(model);
    solver.reasoners.diff.config = stn_config;
    solver
}

struct SimpleMapSolver<Lbl: Label> {
    s: Solver<Lbl>,
    soft_constrs_reif_lits: Arc<BTreeSet<Lit>>,
    to_maximize: IAtom,
}

impl<Lbl: Label> SimpleMapSolver<Lbl> {
    fn new(mut model: Model<Lbl>, soft_constrs_reif_lits: Arc<BTreeSet<Lit>>) -> Self {
        model.enforce(or((*soft_constrs_reif_lits).clone().into_iter().collect_vec()), []);

        let to_maximize = IAtom::from(model.state.new_var(0, INT_CST_MAX));
        let sum_ = LinearSum::of(
            (*soft_constrs_reif_lits)
                .clone()
                .into_iter()
                .map(|l| IAtom::from(l.variable()))
                .collect_vec(),
        );
        model.enforce(sum_.clone().leq(to_maximize), []);
        model.enforce(sum_.clone().geq(to_maximize), []);

        SimpleMapSolver::<Lbl> {
            s: create_solver(model),
            soft_constrs_reif_lits,
            to_maximize,
        }
    }
}

impl<Lbl: Label> MapSolver<Lbl> for SimpleMapSolver<Lbl> {
    fn find_unexplored_seed(&mut self) -> Option<BTreeSet<Lit>> {
        match self.s.maximize(self.to_maximize).unwrap() {
            Some((_, best_assignment)) => {
                let seed = Some(
                    self.soft_constrs_reif_lits
                        .iter()
                        .filter(|&l| best_assignment.entails(*l))
                        .cloned()
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

    fn block_down(&mut self, frompoint: &BTreeSet<Lit>) {
        let complement = self.soft_constrs_reif_lits.difference(frompoint).cloned().collect_vec();
        self.s.enforce(or(complement), []);
    }

    fn block_up(&mut self, frompoint: &BTreeSet<Lit>) {
        let neg = frompoint.iter().map(|&l| !l).collect_vec();
        self.s.enforce(or(neg), []);
    }

    fn get_internal_solver(&mut self) -> &mut Solver<Lbl> {
        &mut self.s
    }
}

struct SimpleSubsetSolver<Lbl: Label> {
    s: Solver<Lbl>,
    soft_constrs_reif_lits: Arc<BTreeSet<Lit>>,
    last_unsat_core: BTreeSet<Lit>,
}

impl<Lbl: Label> SimpleSubsetSolver<Lbl> {
    fn new(model: Model<Lbl>, soft_constrs_reif_lits: Arc<BTreeSet<Lit>>) -> Self {
        SimpleSubsetSolver::<Lbl> {
            s: create_solver(model),
            soft_constrs_reif_lits,
            last_unsat_core: BTreeSet::new(),
        }
    }
}

impl<Lbl: Label> SubsetSolver<Lbl> for SimpleSubsetSolver<Lbl> {
    fn check_seed_sat(&mut self, seed: &BTreeSet<Lit>) -> bool {
        // FIXME warm-start / solution hints optimization should go here... right ?
        let res = self
            .s
            .solve_with_assumptions(seed.clone())
            .expect("Solver interrupted...");
        self.s.reset();
        if let Err(unsat_core) = res {
            self.last_unsat_core.clear();
            self.last_unsat_core.extend(unsat_core.literals());
            false
        } else {
            true
        }
    }

    fn grow(&mut self, seed: &BTreeSet<Lit>) -> (BTreeSet<Lit>, Option<BTreeSet<Lit>>) {
        let mut mss_lits = seed.clone();
        for &lit in (*self.soft_constrs_reif_lits).clone().difference(seed) {
            mss_lits.insert(lit);
            if !self.check_seed_sat(&mss_lits) {
                mss_lits.remove(&lit);
            }
        }
        let mcs_lits: BTreeSet<Lit> = self.soft_constrs_reif_lits.difference(&mss_lits).cloned().collect();
        (mss_lits, Some(mcs_lits))
    }

    fn shrink(&mut self, seed: &BTreeSet<Lit>) -> BTreeSet<Lit> {
        let mut mus_lits: BTreeSet<Lit> = seed.clone();
        for &lit in seed {
            if !mus_lits.contains(&lit) {
                continue;
            }
            mus_lits.remove(&lit);
            if !self.check_seed_sat(&mus_lits) {
                mus_lits = self.last_unsat_core.clone();
            } else {
                debug_assert!(!mus_lits.contains(&lit));
                mus_lits.insert(lit);
            }
        }
        mus_lits
    }

    fn get_internal_solver(&mut self) -> &mut Solver<Lbl> {
        &mut self.s
    }
}

pub struct SimpleMarco<Lbl: Label> {
    soft_constrs_reif_lits: Arc<BTreeSet<Lit>>,
    config: MusMcsEnumerationConfig,
    map_solver: SimpleMapSolver<Lbl>,
    subset_solver: SimpleSubsetSolver<Lbl>,
    result: MusMcsEnumerationResult<Lbl>,
    seed: BTreeSet<Lit>,
}

impl<Lbl: Label> Marco<Lbl> for SimpleMarco<Lbl> {
    fn new<Expr: Reifiable<Lbl> + Copy>(
        model: Model<Lbl>,
        soft_constrs: Vec<Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let mut map_model = model.clone();
        let mut subset_model = model.clone();

        let mapping = soft_constrs
            .clone()
            .into_iter()
            .map(|_| {
                let _a = map_model.state.new_var(0, 1).geq(1);
                let _b = subset_model.state.new_var(0, 1).geq(1);
                debug_assert_eq!(_a, _b);
                _b
            })
            .collect_vec();
        let soft_constrs_reif_lits = Arc::new(mapping.clone().into_iter().collect::<BTreeSet<Lit>>());

        for (soft_constr_reif_lit, soft_constr) in soft_constrs_reif_lits.iter().zip_eq(soft_constrs) {
            subset_model.bind(soft_constr, *soft_constr_reif_lit);
        }
        let result = MusMcsEnumerationResult::<Lbl> {
            soft_constrs_reifs: SoftConstraintsReifications {
                models: Arc::new(vec![subset_model.clone()]),
                mapping,
            },
            muses_reif_lits: if config.return_muses {
                Some(Vec::<BTreeSet<Lit>>::new())
            } else {
                None
            },
            mcses_reif_lits: if config.return_mcses {
                Some(Vec::<BTreeSet<Lit>>::new())
            } else {
                None
            },
        };
        debug_assert_eq!(result.muses_reif_lits.is_some(), config.return_muses);
        debug_assert_eq!(result.mcses_reif_lits.is_some(), config.return_mcses);

        let map_solver = SimpleMapSolver::<Lbl>::new(map_model, soft_constrs_reif_lits.clone());
        let subset_solver = SimpleSubsetSolver::<Lbl>::new(subset_model, soft_constrs_reif_lits.clone());

        Self {
            soft_constrs_reif_lits,
            config,
            map_solver,
            subset_solver,
            result,
            seed: BTreeSet::new(),
        }
    }

    fn reset_result(&mut self) {
        if let Some(ref mut v) = self.result.muses_reif_lits {
            v.clear();
        }
        if let Some(ref mut v) = self.result.mcses_reif_lits {
            v.clear();
        }
    }

    fn clone_result(&self) -> MusMcsEnumerationResult<Lbl> {
        self.result.clone()
    }

    fn get_result(&self) -> &MusMcsEnumerationResult<Lbl> {
        &self.result
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
        if let Some(ref mut v) = self.result.mcses_reif_lits {
            let (mss_lits, mcs_lits) = self.subset_solver.grow(&self.seed);
            self.map_solver.block_down(&mss_lits);
            v.push(mcs_lits.unwrap());
        } else {
            // from Ignace Bleukx's implementation:
            // find more MCSes, **disjoint** from this one, similar to "optimal_mus" in mus.py
            // can only be done when MCSes do not have to be returned as there is no guarantee
            // the MCSes encountered during enumeration are "new" MCSes
            let mut sat_subset_lits: BTreeSet<Lit> = (*self.soft_constrs_reif_lits)
                .clone()
                .into_iter()
                .filter(|&l| self.subset_solver.s.model.entails(!l))
                .collect();
            self.map_solver
                .get_internal_solver()
                .model
                .enforce(or(sat_subset_lits.clone().into_iter().collect_vec()), []);
            while self.subset_solver.check_seed_sat(&sat_subset_lits) {
                let s2: BTreeSet<Lit> = self
                    .soft_constrs_reif_lits
                    .iter()
                    .filter(|&l| self.subset_solver.s.model.entails(*l))
                    .cloned()
                    .collect();
                let new_mcs_lits = self.soft_constrs_reif_lits.difference(&s2).cloned();
                sat_subset_lits.extend(new_mcs_lits.clone());
                self.map_solver.s.model.enforce(or(new_mcs_lits.collect_vec()), []);
            }
        }
    }

    fn do_case_seed_unsat(&mut self) {
        let mus_lits = self.subset_solver.shrink(&self.seed);
        self.map_solver.block_up(&mus_lits);
        if let Some(ref mut v) = self.result.muses_reif_lits {
            v.push(mus_lits);
        }
    }
}
