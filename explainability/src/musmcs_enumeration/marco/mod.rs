mod subsolvers;

use aries::backtrack::Backtrack;
use aries::core::Lit;
use aries::model::Label;
use aries::reif::Reifiable;
use aries::solver::parallel::{ParSolver, SolverResult};
use aries::solver::{Exit, Solver, UnsatCore};

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::musmcs_enumeration::{MusMcsEnumerationConfig, MusMcsEnumerationResult};
use subsolvers::{MapSolver, SubsetSolver};

use itertools::Itertools;

#[allow(dead_code)]
fn find_unsat_core_fn_brute<Lbl: Label>(
    solver: &mut Solver<Lbl>,
    seed: &BTreeSet<Lit>,
) -> Result<Result<(), UnsatCore>, Exit> {
    let res = solver
        .solve_with_assumptions(seed.iter().copied())
        .map(|r| r.map(|_| ()));
    solver.reset();
    res
}

fn find_unsat_core_fn_default<Lbl: Label>(
    solver: &mut Solver<Lbl>,
    seed: &BTreeSet<Lit>,
) -> Result<Result<(), UnsatCore>, Exit> {
    if let Err((_, unsat_core)) = { solver.incremental_push_all(seed.iter().copied()) } {
        solver.reset();
        Ok(Err(unsat_core))
    } else {
        let res = solver.incremental_solve().map(|r| r.map(|_| ()));
        solver.reset_search();
        res
    }
}

#[allow(dead_code)]
fn find_unsat_core_fn_parallel_solver<Lbl: Label>(
    solver: &mut Solver<Lbl>,
    seed: &BTreeSet<Lit>,
) -> Result<Result<(), UnsatCore>, Exit> {
    if let Err((_, unsat_core)) = { solver.incremental_push_all(seed.iter().copied()) } {
        solver.reset();
        Ok(Err(unsat_core))
    } else {
        match ParSolver::new(Box::new(solver.clone()), 4, |_, _| ())
            .incremental_solve(Some(Instant::now() + Duration::from_secs_f64(90.0)))
        {
            SolverResult::Unsat(unsat_core) => Ok(Err(unsat_core.unwrap())),
            _ => Ok(Ok(())),
        }
    }
}

type FindUnsatCoreFn<Lbl> = fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>;

pub struct Marco<Lbl: Label> {
    map_solver: MapSolver,
    subset_solver: SubsetSolver<Lbl>,
    pub soft_constraints_reifications: Arc<BTreeSet<Lit>>,
    seed: BTreeSet<Lit>,
    result: MusMcsEnumerationResult,
    find_unsat_core_fn: FindUnsatCoreFn<Lbl>,
}

impl<Lbl: Label> Marco<Lbl> {
    pub fn with_soft_constraints<Expr: Reifiable<Lbl>>(
        mut solver: Solver<Lbl>,
        soft_constraints: impl IntoIterator<Item = Expr>,
        config: MusMcsEnumerationConfig,
        find_unsat_core_fn: Option<FindUnsatCoreFn<Lbl>>,
    ) -> Self {
        let soft_constraints_reifications = soft_constraints
            .into_iter()
            .map(|expr| solver.reify(expr))
            .collect_vec();

        Self::with_reified_soft_constraints(solver, soft_constraints_reifications, config, find_unsat_core_fn)
    }

    pub fn with_reified_soft_constraints(
        solver: Solver<Lbl>,
        soft_constraints_reifications: impl IntoIterator<Item = Lit>,
        config: MusMcsEnumerationConfig,
        find_unsat_core_fn: Option<FindUnsatCoreFn<Lbl>>,
    ) -> Self {
        let result = MusMcsEnumerationResult {
            muses: config.return_muses.then(Vec::<BTreeSet<Lit>>::new),
            mcses: config.return_mcses.then(Vec::<BTreeSet<Lit>>::new),
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
            find_unsat_core_fn: find_unsat_core_fn.unwrap_or(find_unsat_core_fn_default),
        }
    }

    pub fn get_expr_reification<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        self.subset_solver.solver.model.check_reified(expr)
    }

    pub fn run(&mut self) -> Result<MusMcsEnumerationResult, Exit> {
        self.run_with(self.find_unsat_core_fn)
    }

    fn run_with(
        &mut self,
        find_unsat_core_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<MusMcsEnumerationResult, Exit> {
        self.reset_result();

        while let Some(next_seed) = self.map_solver.find_unexplored_seed()? {
            self.seed = next_seed;
            if self
                .subset_solver
                .check_subset(&self.seed, &find_unsat_core_fn)?
                .is_ok()
            {
                if let Some(ref mut mcses) = self.result.mcses {
                    let (mss, mcs) = self.subset_solver.grow(&self.seed, &find_unsat_core_fn)?;
                    self.map_solver.block_down(&mss);
                    mcses.push(mcs.unwrap());
                } else {
                    self.case_seed_sat_only_muses_optimization(&find_unsat_core_fn)?;
                }
            } else {
                let mus = self.subset_solver.shrink(&self.seed, &find_unsat_core_fn)?;
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

    fn case_seed_sat_only_muses_optimization(
        &mut self,
        find_unsat_core_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
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
        while self
            .subset_solver
            .check_subset(&sat_subset, &find_unsat_core_fn)?
            .is_ok()
        {
            let new_sat_subset: BTreeSet<Lit> = self
                .soft_constraints_reifications
                .iter()
                .filter(|&&l| self.subset_solver.solver.model.state.entails(l))
                .copied()
                .collect();
            self.map_solver.block_down(&new_sat_subset);

            let new_corr_subset = self.soft_constraints_reifications.difference(&new_sat_subset);
            sat_subset.extend(new_corr_subset.clone());

            // Same optimization as (*) above
            if let Some(&lit) = new_corr_subset.take(2).fold(None, |opt, item| opt.xor(Some(item))) {
                self.subset_solver.necessarily_in_all_muses.insert(lit);
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
    type Solver = aries::solver::Solver<Lbl>;
    type Marco = super::Marco<Lbl>;

    use crate::musmcs_enumeration::MusMcsEnumerationConfig;

    #[test]
    fn test_simple_marco_simple() {
        let mut model: Model = Model::new();
        let x0 = model.new_ivar(0, 10, "x0");
        let x1 = model.new_ivar(0, 10, "x1");
        let x2 = model.new_ivar(0, 10, "x2");

        let soft_constrs = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2)];
        let mut simple_marco = Marco::with_soft_constraints(
            Solver::new(model),
            soft_constrs.clone(),
            MusMcsEnumerationConfig {
                return_muses: true,
                return_mcses: true,
            },
            None,
        );
        let soft_constrs_reif_lits = soft_constrs
            .into_iter()
            .map(|expr| simple_marco.get_expr_reification(expr))
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
