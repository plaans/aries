use std::collections::BTreeSet;

use aries::core::Lit;
use aries::model::{Label, Model};
use aries::reif::Reifiable;
use aries::solver::{Exit, UnsatCore};
use itertools::Itertools;

use crate::musmcs_enumeration::{Mcs, Mus};

pub trait SubsetSolverImpl<Lbl: Label> {
    fn get_model(&mut self) -> &mut Model<Lbl>;
    fn find_unsat_core(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>;
}

pub struct SubsetSolver<Lbl: Label> {
    /// These literals reify / represent soft constraints in the subset solver (this struct).
    literals: BTreeSet<Lit>,
    literals_known_to_be_necessarily_in_every_mus: BTreeSet<Lit>,

    subset_solver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
}

impl<Lbl: Label> SubsetSolver<Lbl> {
    pub fn new(
        soft_constraints_reif_literals: impl IntoIterator<Item = Lit>,
        mut subset_solver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
    ) -> Self {
        let literals = soft_constraints_reif_literals.into_iter().collect::<BTreeSet<Lit>>();
        assert!(literals.iter().all(|&l| subset_solver_impl.get_model().check_reified_any(l).is_some()));

        Self {
            literals,
            literals_known_to_be_necessarily_in_every_mus: BTreeSet::new(),
            subset_solver_impl,
        }
    }

    pub fn get_expr_reification<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        self.subset_solver_impl.get_model().check_reified_any(expr)
    }

    pub fn get_soft_constraints_reif_literals(&self) -> &BTreeSet<Lit> {
        &self.literals
    }

    pub fn get_soft_constraints_known_to_be_necessarily_in_every_mus(&self) -> &BTreeSet<Lit> {
        &self.literals_known_to_be_necessarily_in_every_mus
    }

    pub fn register_soft_constraint_as_necessarily_in_every_mus(&mut self, soft_constraint_reif_lit: Lit) {
        self.literals_known_to_be_necessarily_in_every_mus
            .insert(soft_constraint_reif_lit);
    }

    fn find_unsat_core(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit> {
        self.subset_solver_impl.find_unsat_core(subset)
    }

    pub fn check_subset(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), BTreeSet<Lit>>, Exit> {
        let res = self.find_unsat_core(subset)?;
        // NOTE: any resetting (or not!) of assumptions of the solver is to be done in `find_unsat_core`
        Ok(res.map_err(|unsat_core| {
            unsat_core
                .literals()
                .iter()
                .chain(self.get_soft_constraints_known_to_be_necessarily_in_every_mus())
                .copied()
                .collect()
        }))
    }

    pub fn grow(&mut self, subset: &BTreeSet<Lit>) -> Result<(BTreeSet<Lit>, Mcs), Exit> {
        let mut mss = subset.clone();
        let complement = self
            .get_soft_constraints_reif_literals()
            .difference(subset)
            .copied()
            .collect_vec();
        for lit in complement {
            mss.insert(lit);
            if self.check_subset(&mss)?.is_err() {
                mss.remove(&lit);
            }
        }
        let mcs: BTreeSet<Lit> = self
            .get_soft_constraints_reif_literals()
            .difference(&mss)
            .copied()
            .collect();

        // If the found correction set only has 1 element,
        // then that element is added to those that are known to be in all unsatisfiable sets.
        if let Ok(&single_lit_mcs) = mcs.iter().exactly_one() {
            self.register_soft_constraint_as_necessarily_in_every_mus(single_lit_mcs);
        }
        Ok((mss, mcs))
    }

    pub fn shrink(&mut self, subset: &BTreeSet<Lit>) -> Result<Mus, Exit> {
        let mut mus: BTreeSet<Lit> = subset.clone();
        for &lit in subset {
            if !mus.contains(&lit) {
                continue;
            }
            // Optimization: if the literal has been determined to belong to all muses,
            // no need to check if, without it, the set would be satisfiable (because it obviously would be).
            if self
                .get_soft_constraints_known_to_be_necessarily_in_every_mus()
                .contains(&lit)
            {
                continue;
            }
            mus.remove(&lit);
            if let Err(unsat_core) = self.check_subset(&mus)? {
                mus = unsat_core;
            } else {
                debug_assert!(!mus.contains(&lit));
                mus.insert(lit);
            }
        }
        Ok(mus)
    }
}
