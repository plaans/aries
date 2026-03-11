use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{Problem, SolveResult, SolveStatus, SolverID, comp::RunWithRef, metric::Metric};

#[derive(Clone, Default, Debug)]
pub struct ResultCollection {
    pub solvers: HashSet<SolverID>,
    pub results: HashMap<Problem, ProblemResults>,
}

impl ResultCollection {
    pub fn add_solver(&mut self, solver: SolverID, results: impl IntoIterator<Item = Rc<SolveResult>>) {
        assert!(!self.solvers.contains(&solver));
        self.solvers.insert(solver.clone());
        for res in results {
            let pb = &res.problem;
            self.results
                .entry(pb.clone())
                .or_insert_with(|| ProblemResults::empty(pb.clone()))
                .add_result(solver.clone(), res);
        }
    }

    pub fn retain(&mut self, mut pred: impl FnMut(&Problem) -> bool) {
        self.results.retain(|k, _v| pred(k));
    }

    pub fn with_data_for_all_solvers(mut self) -> Self {
        self.results.retain(|_, runs| runs.results.len() == self.solvers.len());
        self
    }

    /// Retains only instances that were solved by all solvers.
    pub fn easy(self) -> Self {
        let mut x = self.with_data_for_all_solvers();
        x.results
            .retain(|_, runs| runs.results.values().all(|r| r.status == SolveStatus::Solved));
        x
    }

    /// Retains only instances that at least one solver could not solve.
    pub fn hard(self) -> Self {
        let mut x = self.with_data_for_all_solvers();
        x.results
            .retain(|_, runs| runs.results.values().any(|r| r.status != SolveStatus::Solved));
        x
    }

    pub fn comparison(&self, main: &SolverID, reference: &SolverID) -> Vec<RunWithRef> {
        assert!(
            self.solvers.contains(main),
            "Missing solver '{main} in {:?}",
            self.solvers
        );
        assert!(
            self.solvers.contains(reference),
            "Missing solver '{reference} in {:?}",
            self.solvers
        );
        self.results
            .values()
            .filter_map(|r| {
                r.get_solver(main).map(|main| RunWithRef {
                    run: main,
                    reference: r.get_solver(reference),
                })
            })
            .collect()
    }

    pub fn measures<'a, M: Metric + 'static>(
        &'a self,
        m: M,
    ) -> impl Iterator<Item = (&'a Problem, &'a SolverID, M::T)> + 'a {
        self.results
            .values()
            .flat_map(|runs| self.solvers.iter().map(move |s| (runs, s)))
            .map(move |(runs, solver)| {
                let Some(run) = runs.get_solver(solver) else {
                    panic!("Solver '{solver}' is missing in runs: {runs:?}");
                };
                let measure = m.compute(&run, runs);
                (&runs.problem, solver, measure)
            })
    }
}

#[derive(Clone, Debug)]
pub struct ProblemResults {
    pub problem: Problem,
    pub results: HashMap<SolverID, Rc<SolveResult>>,
}

impl ProblemResults {
    pub fn empty(problem: Problem) -> Self {
        Self {
            problem,
            results: Default::default(),
        }
    }

    pub fn get_solver(&self, solver: &SolverID) -> Option<Rc<SolveResult>> {
        self.results.get(solver).cloned()
    }

    pub fn add_result(&mut self, solver: SolverID, result: Rc<SolveResult>) {
        assert_eq!(&self.problem, &result.problem);
        assert!(
            !self.results.contains_key(&solver),
            "Solver: {solver}, previous: {:?}",
            self.results
        );

        self.results.insert(solver, result);
    }
}
