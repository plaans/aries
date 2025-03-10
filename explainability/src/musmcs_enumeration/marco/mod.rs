mod subsolvers;
use aries::backtrack::Backtrack;
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::solver::{Exit, Solver, UnsatCore};
use subsolvers::{MapSolver, SubsetSolver};

use crate::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};

use aries::core::Lit;
use aries::model::{Label, Model};
use aries::reif::Reifiable;
use std::collections::BTreeSet;
use std::sync::Arc;

use itertools::Itertools;

pub struct Marco<Lbl: Label> {
    pub soft_constraints_reifications: Arc<BTreeSet<Lit>>,
    map_solver: MapSolver,
    subset_solver: SubsetSolver<Lbl>,
    seed: BTreeSet<Lit>,
    result: MusMcsEnumerationResult,
}

impl<Lbl: Label> Marco<Lbl> {
    pub fn with_soft_constraints<Expr: Reifiable<Lbl>>(
        mut solver: Solver<Lbl>,
        soft_constraints: impl IntoIterator<Item = Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let soft_constraints_reifications = soft_constraints
            .into_iter()
            .map(|expr| solver.reify(expr))
            .collect_vec();

        Self::with_reified_soft_constraints(solver, soft_constraints_reifications, config)
    }

    pub fn with_reified_soft_constraints(
        solver: Solver<Lbl>,
        soft_constraints_reifications: impl IntoIterator<Item = Lit>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let result = MusMcsEnumerationResult {
            muses: config.return_muses.then(|| Vec::<BTreeSet<Lit>>::new()),
            mcses: config.return_mcses.then(|| Vec::<BTreeSet<Lit>>::new()),
        };
        let soft_constraints_reifications = Arc::new(BTreeSet::from_iter(soft_constraints_reifications));

        let map_solver = MapSolver::new(soft_constraints_reifications.iter().copied());
        let subset_solver = SubsetSolver::<Lbl>::new(solver, soft_constraints_reifications.clone());

        Self {
            soft_constraints_reifications,
            seed: BTreeSet::new(),
            map_solver,
            subset_solver,
            result,
        }
    }

    pub fn get_expr_reification<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        self.subset_solver.solver.model.check_reified(expr)
    }

    pub fn run(
        &mut self,
        solve_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<MusMcsEnumerationResult, Exit> {
        self.reset_result();

        while let Some(next_seed) = self.map_solver.find_unexplored_seed()? {
            self.seed = next_seed;
            if self.subset_solver.check_seed_sat(&self.seed, &solve_fn)? {
                if let Some(ref mut mcses) = self.result.mcses {
                    let (mss, mcs) = self.subset_solver.grow(&self.seed, &solve_fn)?;
                    self.map_solver.block_down(&mss);
                    mcses.push(mcs.unwrap());
                } else {
                    self.case_seed_sat_only_muses_optimization(&solve_fn)?;
                }
            } else {
                let mus = self.subset_solver.shrink(&self.seed, &solve_fn)?;
                self.map_solver.block_up(&mus);
                if let Some(ref mut muses) = self.result.muses {
                    muses.push(mus);
                }
            }
        }
        Ok(self.clone_result())

    }

    fn reset_result(&mut self) {
        if let Some(ref mut muses) = self.result.muses {
            muses.clear();
        }
        if let Some(ref mut mcses) = self.result.mcses {
            mcses.clear();
        }
    }

    fn clone_result(&self) -> MusMcsEnumerationResult {
        self.result.clone()
    }

    // fn find_unexplored_seed(&mut self) -> bool {
    fn case_seed_sat_only_muses_optimization(
        &mut self,
        solve_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<(), Exit> {
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
            .soft_constraints_reifications
            .difference(&sat_subset)
            .take(2)
            .fold(None, |opt, item| opt.xor(Some(item)))
        {
            self.subset_solver.necessarily_in_all_muses.insert(lit);
        }

        // Grow the sat subset as much as possible (i.e. until unsatisfiability
        // by extending it with each correction set discovered.
        // while self
        while solve_fn(&mut self.subset_solver.solver, &sat_subset)?.is_ok()
        {
            let new_sat_subset: BTreeSet<Lit> = self
                .soft_constraints_reifications
                .iter()
                .filter(|&&l| self.subset_solver.solver.model.state.entails(l))
                .copied()
                .collect();
            self.map_solver.block_down(&new_sat_subset);

            let new_corr_subset = self.soft_constraints_reifications.difference(&new_sat_subset).into_iter();
            sat_subset.extend(new_corr_subset.clone());

            // Same optimization as (*) above
            if let Some(&lit) = new_corr_subset.take(2).fold(None, |opt, item| opt.xor(Some(item))) {
                self.subset_solver.necessarily_in_all_muses.insert(lit);
            }

            self.subset_solver.solver.reset();
        }
        self.subset_solver.solver.reset();
        Ok(())
    }
}

pub struct SimpleMarco<Lbl: Label> {
    marco: Marco<Lbl>,
}

impl<Lbl: Label> SimpleMarco<Lbl> {
    pub fn with_soft_constraints<Expr: Reifiable<Lbl>>(
        model: Model<Lbl>,
        soft_constraints: impl IntoIterator<Item = Expr>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let mut solver = Solver::<Lbl>::new(model);
        solver.reasoners.diff.config = StnConfig {
            theory_propagation: TheoryPropagationLevel::Full,
            ..Default::default()
        };
        Self { marco: Marco::with_soft_constraints(solver, soft_constraints, config) }
    }

    pub fn with_reified_soft_constraints(
        model: Model<Lbl>,
        soft_constraints_reifications: impl IntoIterator<Item = Lit>,
        config: MusMcsEnumerationConfig,
    ) -> Self {
        let mut solver = Solver::<Lbl>::new(model);
        solver.reasoners.diff.config = StnConfig {
            theory_propagation: TheoryPropagationLevel::Full,
            ..Default::default()
        };
        Self { marco: Marco::with_reified_soft_constraints(solver, soft_constraints_reifications, config) }
    }

    pub fn run(
        &mut self,
    ) -> Result<MusMcsEnumerationResult, Exit> {
        let solve_fn = |solver: &mut Solver<Lbl>, seed: &BTreeSet<Lit>| {
            solver.solve_with_assumptions(seed.iter().copied()).map(|r| r.map(|_| ()))
        };
        self.marco.run(solve_fn)
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use aries::model::lang::expr::lt;
    use itertools::Itertools;

    type Lbl = &'static str;

    type Model = aries::model::Model<Lbl>;
    type SimpleMarco = super::SimpleMarco<Lbl>;

    use crate::musmcs_enumeration::MusMcsEnumerationConfig;

    type Model = aries::model::Model<&'static str>;
    type SimpleMarco = crate::musmcs_enumeration::marco::simple_marco::SimpleMarco<&'static str>;

    #[test]
    fn test_simple_marco_simple() {
        let mut model: Model = Model::new();
        let x0 = model.new_ivar(0, 10, "x0");
        let x1 = model.new_ivar(0, 10, "x1");
        let x2 = model.new_ivar(0, 10, "x2");

        let soft_constrs = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2)];
        let mut simple_marco = SimpleMarco::with_soft_constraints(
            model,
            soft_constrs.clone(),
            MusMcsEnumerationConfig {
                return_muses: true,
                return_mcses: true,
            },
        );
        let soft_constrs_reif_lits = soft_constrs
            .into_iter()
            .map(|expr| simple_marco.marco.get_expr_reification(expr))
            .collect_vec();

        let res = simple_marco.run().unwrap();
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
