use aries::core::Lit;
use aries::model::Label;
use aries::reif::Reifiable;
use aries::solver::Exit;

use std::{collections::BTreeSet, time::Instant};

use itertools::Itertools;

use crate::musmcs_enumeration::{Mcs, Mus, MusMcsEnumResult};
use subsolvers::{MapSolver, MapSolverMode, SubsetSolver, SubsetSolverImpl, SubsetSolverOptiMode};

pub mod subsolvers;

pub struct Marco<Lbl: Label> {
    msolver: MapSolver,
    /// The subset solver, sometimes also simply called "constraint solver". Hence the name `csolver`.
    csolver: SubsetSolver<Lbl>,
}

impl<Lbl: Label> Marco<Lbl> {
    pub fn with_soft_constraints_full_reif<Expr: Reifiable<Lbl>>(
        soft_constraints: impl IntoIterator<Item = Expr>,
        mut csolver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
        msolver_mode: MapSolverMode,
    ) -> Self {
        let soft_constraints_reiflits = soft_constraints
            .into_iter()
            .map(|expr| csolver_impl.get_model().reify(expr))
            .collect_vec();

        Self::with_reified_soft_constraints(soft_constraints_reiflits, csolver_impl, msolver_mode)
    }

    pub fn with_soft_constraints_half_reif<Expr: Reifiable<Lbl>>(
        soft_constraints: impl IntoIterator<Item = Expr>,
        mut csolver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
        msolver_mode: MapSolverMode,
    ) -> Self {
        let soft_constraints_reiflits = soft_constraints
            .into_iter()
            .map(|expr| csolver_impl.get_model().half_reify(expr))
            .collect_vec();

        Self::with_reified_soft_constraints(soft_constraints_reiflits, csolver_impl, msolver_mode)
    }

    /// NOTE: Both half-reification and full reification literals are supported, including in the same model.
    pub fn with_reified_soft_constraints(
        soft_constraints_reiflits: impl IntoIterator<Item = Lit> + Clone,
        csolver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
        msolver_mode: MapSolverMode,
    ) -> Self {
        let msolver = MapSolver::new(soft_constraints_reiflits.clone(), msolver_mode);
        let csolver = SubsetSolver::<Lbl>::new(soft_constraints_reiflits, csolver_impl);

        Self { msolver, csolver }
    }

    pub fn get_expr_reif<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        self.csolver.get_expr_reif(expr)
    }

    pub fn get_soft_constraints_reif_lits(&self) -> &BTreeSet<Lit> {
        self.csolver.get_soft_constraints_reif_lits()
    }

    pub fn run(&mut self, on_mus_found: Option<fn(&Mus)>, on_mcs_found: Option<fn(&Mcs)>) -> MusMcsEnumResult {
        let mut muses = Vec::<Mus>::new();
        let mut mcses = Vec::<Mcs>::new();

        let start = Instant::now();

        let complete = self
            ._run(
                &mut muses,
                &mut mcses,
                on_mus_found,
                on_mcs_found,
                SubsetSolverOptiMode::default(),
            )
            .is_ok();

        debug_assert!(muses.iter().all_unique());
        debug_assert!(mcses.iter().all_unique());

        MusMcsEnumResult {
            muses,
            mcses,
            complete: Some(complete),
            run_time: Some(start.elapsed()),
        }
    }

    fn _run(
        &mut self,
        muses: &mut Vec<Mus>,
        mcses: &mut Vec<Mcs>,
        on_mus_found: Option<fn(&Mus)>,
        on_mcs_found: Option<fn(&Mcs)>,
        optional_optim: SubsetSolverOptiMode,
    ) -> Result<(), Exit> {
        while let Some(seed) = self.msolver.find_unexplored_seed()? {
            if self.csolver.check_subset(&seed)?.is_ok() {
                let (_, mcs) = self.csolver.grow(&seed, (optional_optim, &mut self.msolver))?;
                self.msolver.block_down(&mcs);

                debug_assert!(mcses.iter().all(|set| !mcs.is_subset(set) && !set.is_subset(&mcs)));
                if !mcs.is_empty() {
                    on_mcs_found.unwrap_or(|_| ())(&mcs);
                    mcses.push(mcs);
                }
            } else {
                // debug_assert!(self.msolver.seed_is_unexplored(&seed));

                let mus = self.csolver.shrink(&seed, (optional_optim, &mut self.msolver))?;
                self.msolver.block_up(&mus);

                debug_assert!(muses.iter().all(|set| !mus.is_subset(set) && !set.is_subset(&mus)));
                if !mus.is_empty() {
                    on_mus_found.unwrap_or(|_| ())(&mus);
                    muses.push(mus);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use aries::backtrack::Backtrack;
    use aries::core::Lit;
    use aries::model::extensions::SavedAssignment;
    use aries::model::lang::expr::{geq, lt};
    use aries::solver::{Exit, UnsatCore};
    use itertools::Itertools;

    type Lbl = &'static str;

    type Model = aries::model::Model<Lbl>;
    type Solver = aries::solver::Solver<Lbl>;
    type Marco = super::Marco<Lbl>;

    use super::subsolvers::MapSolverMode;
    use super::subsolvers::SubsetSolverImpl;

    struct SimpleSubsetSolverImpl {
        solver: Solver,
    }
    impl SimpleSubsetSolverImpl {
        pub fn new(model: Model) -> Self {
            Self {
                solver: Solver::new(model),
            }
        }
    }
    impl SubsetSolverImpl<Lbl> for SimpleSubsetSolverImpl {
        fn get_model(&mut self) -> &mut Model {
            &mut self.solver.model
        }
        fn check_subset(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<Arc<SavedAssignment>, UnsatCore>, Exit> {
            let res = self
                .solver
                .solve_with_assumptions(subset.iter().copied().collect_vec())?;
            self.solver.reset();
            Ok(res)
        }
    }

    #[test]
    fn test_simple_marco_simple() {
        let mut model: Model = Model::new();
        let x0 = model.new_ivar(0, 10, "x0");
        let x1 = model.new_ivar(0, 10, "x1");
        let x2 = model.new_ivar(0, 10, "x2");
        let x3 = model.new_ivar(0, 10, "x3");

        let soft_constraints = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2), lt(x3, 5), geq(x3, 5)];
        let mut simple_marco = Marco::with_soft_constraints_half_reif(
            soft_constraints.clone(),
            Box::new(SimpleSubsetSolverImpl::new(model)),
            MapSolverMode::HighPreferredValues,
        );
        let soft_constraints_reiflits = soft_constraints
            .into_iter()
            .map(|expr| simple_marco.get_expr_reif(expr))
            .collect_vec();

        let res = simple_marco.run(None, None);
        let res_muses = res.muses.into_iter().collect::<BTreeSet<_>>();
        let res_mcses = res.mcses.into_iter().collect::<BTreeSet<_>>();

        let expected_muses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[0].unwrap(),
                soft_constraints_reiflits[1].unwrap(),
                soft_constraints_reiflits[2].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[2].unwrap(),
                soft_constraints_reiflits[3].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[4].unwrap(),
                soft_constraints_reiflits[5].unwrap(),
            ]),
        ]);
        let expected_mcses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[2].unwrap(),
                soft_constraints_reiflits[5].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[0].unwrap(),
                soft_constraints_reiflits[3].unwrap(),
                soft_constraints_reiflits[5].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[1].unwrap(),
                soft_constraints_reiflits[3].unwrap(),
                soft_constraints_reiflits[5].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[2].unwrap(),
                soft_constraints_reiflits[4].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[0].unwrap(),
                soft_constraints_reiflits[3].unwrap(),
                soft_constraints_reiflits[4].unwrap(),
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[1].unwrap(),
                soft_constraints_reiflits[3].unwrap(),
                soft_constraints_reiflits[4].unwrap(),
            ]),
        ]);

        assert_eq!(res_muses, expected_muses);
        assert_eq!(res_mcses, expected_mcses);
    }
}
