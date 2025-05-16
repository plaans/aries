pub mod subsolvers;

use aries::core::Lit;
use aries::model::Label;
use aries::reif::Reifiable;
use aries::solver::Exit;

use std::{collections::BTreeSet, time::Instant};

use crate::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};
use subsolvers::{MapSolver, SubsetSolver, SubsetSolverImpl};

use itertools::Itertools;

pub struct Marco<Lbl: Label> {
    map_solver: MapSolver,
    subset_solver: SubsetSolver<Lbl>,
    config: MusMcsEnumerationConfig,
}

impl<Lbl: Label> Marco<Lbl> {
    pub fn with_soft_constraints<Expr: Reifiable<Lbl>>(
        mut subset_solver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
        soft_constraints: impl IntoIterator<Item = Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self {

        let soft_constraints_reif_literals = soft_constraints
            .into_iter()
            .map(|expr| subset_solver_impl.get_model().reify(expr))
            .collect_vec();

        Self::with_reified_soft_constraints(
            subset_solver_impl,
            soft_constraints_reif_literals,
            config,
        )
    }

    pub fn with_reified_soft_constraints(
        subset_solver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
        soft_constraints_reif_literals: impl IntoIterator<Item = Lit> + Clone,
        config: MusMcsEnumerationConfig,
    ) -> Self {

        let map_solver = MapSolver::new(soft_constraints_reif_literals.clone());
        let subset_solver = SubsetSolver::<Lbl>::new(soft_constraints_reif_literals, subset_solver_impl);

        Self {
            map_solver,
            subset_solver,
            config,
        }
    }

    pub fn get_expr_reification<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        self.subset_solver.get_expr_reification(expr)
    }

    fn get_soft_constraints_reif_literals(&self) -> &BTreeSet<Lit> {
        self.subset_solver.get_soft_constraints_reif_literals()
    }

    #[allow(dead_code)]
    fn get_soft_constraints_known_to_be_necessarily_in_every_mus(&self) -> &BTreeSet<Lit> {
        self.subset_solver.get_soft_constraints_known_to_be_necessarily_in_every_mus()
    }

    fn register_soft_constraint_as_necessarily_in_every_mus(&mut self, soft_constraint_reif_lit: Lit) {
        self.subset_solver.register_soft_constraint_as_necessarily_in_every_mus(soft_constraint_reif_lit)
    }

    pub fn run(&mut self) -> MusMcsEnumerationResult {
        let mut muses = self.config.return_muses.then(Vec::<BTreeSet<Lit>>::new);
        let mut mcses = self.config.return_mcses.then(Vec::<BTreeSet<Lit>>::new);

        let start = Instant::now();

        let complete = self._run(&mut muses, &mut mcses).is_ok();

        debug_assert!(muses.as_ref().is_none_or(|v| v.iter().all_unique()));
        debug_assert!(mcses.as_ref().is_none_or(|v| v.iter().all_unique()));

        MusMcsEnumerationResult {
            muses,
            mcses,
            complete: Some(complete),
            run_time: Some(start.elapsed()),
        }
    }

    fn _run(&mut self, muses: &mut Option<Vec<BTreeSet<Lit>>>, mcses: &mut Option<Vec<BTreeSet<Lit>>>) -> Result<(), Exit> {
        while let Some(next_seed) = self.map_solver.find_unexplored_seed()? {
            let seed = next_seed;
            if self
                .subset_solver
                .check_subset(&seed)?
                .is_ok()
            {
                if let Some(mcses) = mcses {
                    let (mss, mcs) = self.subset_solver.grow(&seed)?;
                    self.map_solver.block_down(&mss);
                    assert!(mcses.iter().all(|known_mcs| !mcs.is_subset(known_mcs) && !known_mcs.is_subset(&mcs)));
                    if !mcs.is_empty() {
                        self.config.on_mcs_found.as_ref().map(|f| f(&mcs));
                        mcses.push(mcs);
                    }
                } else {
                    self.case_seed_sat_only_muses_optimization(&seed)?;
                }
            } else {
                let mus = self.subset_solver.shrink(&seed)?;
                self.map_solver.block_up(&mus);
                if let Some(muses) = muses {
                    assert!(muses.iter().all(|known_mus| !mus.is_subset(known_mus) && !known_mus.is_subset(&mus)));
                    if !mus.is_empty() {
                        self.config.on_mus_found.as_ref().map(|f| f(&mus));
                        muses.push(mus);
                    }
                }
            }
        }
        Ok(())
    }

    fn case_seed_sat_only_muses_optimization(&mut self, seed: &BTreeSet<Lit>) -> Result<(), Exit> {
        // Optimization inspired by the implementation of Ignace Bleukx (in python).
        //
        // If we are not going to return MCSes, we can try to greedily search for
        // more correction subsets, disjoint from this one (the seed).
        //
        // This can only be done when we only intend to return MUSes, not MCSes,
        // because the correction sets we greedily discover with this optimization
        // have no guarantee of being unique / not having been already discovered.

        let mut sat_subset = seed.clone();
        self.map_solver.block_down(&sat_subset);

        // Another optimization (*):
        // If the found correction set only has 1 element,
        // then that element is added to those that are known to be in all unsatisfiable sets.
        if let Ok(&lit) = self
            .get_soft_constraints_reif_literals()
            .difference(&sat_subset)
            .exactly_one()
        {
            self.register_soft_constraint_as_necessarily_in_every_mus(lit);
        }

        // Grow the sat subset as much as possible (i.e. until unsatisfiability
        // by extending it with each correction set discovered.       
        while let Some(new_sat_subset) = self.subset_solver.find_all_sat_with_subset(&sat_subset)? {

            self.map_solver.block_down(&new_sat_subset);

            let new_corr_subset = self.get_soft_constraints_reif_literals().difference(&new_sat_subset);
            sat_subset.extend(new_corr_subset.clone());

            // Same optimization as (*) above
            if let Ok(&lit) = new_corr_subset.exactly_one() {
                self.register_soft_constraint_as_necessarily_in_every_mus(lit);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use aries::model::lang::expr::lt;
    use itertools::Itertools;

    type Lbl = &'static str;

    type Model = aries::model::Model<Lbl>;
    type Marco = super::Marco<Lbl>;
    type SimpleSubsetSolverImpl = super::subsolvers::SimpleSubsetSolverImpl<Lbl>;

    use crate::musmcs_enumeration::MusMcsEnumerationConfig;

    #[test]
    fn test_simple_marco_simple() {
        let mut model: Model = Model::new();
        let x0 = model.new_ivar(0, 10, "x0");
        let x1 = model.new_ivar(0, 10, "x1");
        let x2 = model.new_ivar(0, 10, "x2");

        let soft_constrs = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2)];
        let mut simple_marco = Marco::with_soft_constraints(
            Box::new(SimpleSubsetSolverImpl::new(model)),
            soft_constrs.clone(),
            MusMcsEnumerationConfig {
                return_muses: true,
                return_mcses: true,
                on_mus_found: None,
                on_mcs_found: None,
            },
        );
        let soft_constrs_reif_lits = soft_constrs
            .into_iter()
            .map(|expr| simple_marco.get_expr_reification(expr))
            .collect_vec();

        let res = simple_marco.run();
        let res_muses = res.muses.unwrap().into_iter().collect::<BTreeSet<_>>();
        let res_mcses = res.mcses.unwrap().into_iter().collect::<BTreeSet<_>>();

        let expected_muses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[0].unwrap(),
                soft_constrs_reif_lits[1].unwrap(),
                soft_constrs_reif_lits[2].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[2].unwrap(),
                soft_constrs_reif_lits[3].unwrap(),
            ]),
        ]);
        let expected_mcses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![soft_constrs_reif_lits[2].unwrap()]),
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[0].unwrap(),
                soft_constrs_reif_lits[3].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constrs_reif_lits[1].unwrap(),
                soft_constrs_reif_lits[3].unwrap(),
            ]),
        ]);

        assert_eq!(res_muses, expected_muses);
        assert_eq!(res_mcses, expected_mcses);
    }
}
