mod lp;

pub(crate) use lp::LpRelaxIncr;

#[cfg(test)]
mod tests {

    use aries_solver::reasoners::lp::LpOptions;

    use crate::lprelax::tests::visitall::{VisitAllLine, build_and_encode_visitall_line};
    use crate::lprelax::wrappers::LpRelaxIncr;

    /// No limit on the pivots of the LP, so that it can conclude at the root.
    fn unlimited_pivots() -> LpOptions {
        LpOptions {
            initially_allowed_pivots: f64::INFINITY,
            ..Default::default()
        }
    }

    #[test]
    fn test_visitall_line_incr() {
        let sat_pb = &VisitAllLine {
            num_locs: 4,
            num_moves: 3,
        };
        let encoder = build_and_encode_visitall_line(sat_pb, false);
        let model = encoder.sched.clone().encode();

        {
            println!("Sat instance with lprelax (lprelax mustn't deem it unsat)");

            let reasoner = LpRelaxIncr::new(encoder, unlimited_pivots());
            let mut solver = aries_solver::solver::Solver::with_extra_reasoners(model, vec![Box::new(reasoner)]);

            assert!(
                solver
                    .solve(aries_solver::solver::SearchLimit::None)
                    .is_ok_and(|sol| sol.is_some())
            );
        }

        let unsat_pb = &VisitAllLine {
            num_locs: 5,
            num_moves: 3,
        };
        let encoder = build_and_encode_visitall_line(unsat_pb, false);
        let model = encoder.sched.clone().encode();

        {
            println!("Unsat instance without lprelax (num decisions must be > 0)");

            let mut solver = aries_solver::solver::Solver::with_extra_reasoners(model.clone(), vec![]);

            assert!(
                solver
                    .solve(aries_solver::solver::SearchLimit::None)
                    .is_ok_and(|sol| sol.is_none())
            );
            assert!(solver.stats.num_decisions > 0);

            let reasoner = LpRelaxIncr::new(encoder, unlimited_pivots());

            println!("Unsat instance with lprelax (num decisions must be = 0, thanks to lprelax)");

            let mut solver = aries_solver::solver::Solver::with_extra_reasoners(model, vec![Box::new(reasoner)]);

            assert!(
                solver
                    .solve(aries_solver::solver::SearchLimit::None)
                    .is_ok_and(|sol| sol.is_none())
            );
            assert!(solver.stats.num_decisions == 0);
        }
    }

    /// A solution found without lprelax must remain consistent with lprelax:
    ///
    /// A new solve, with lprelax, and with the solution's assignments as assumptions, must be feasible / sat.
    #[test]
    fn test_visitall_line_incr_accepts_solutions() {
        // (4 moves availble, so one of them may be left out of the plan.)
        let num_moves = 4;

        let pb = &VisitAllLine { num_locs: 4, num_moves };
        let encoder = build_and_encode_visitall_line(pb, false);

        let model = encoder.sched.clone().encode();
        let mut solver = aries_solver::solver::Solver::with_extra_reasoners(model.clone(), vec![]);

        let solution = solver.solve(aries_solver::solver::SearchLimit::None).unwrap();

        assert!(solution.is_some());
        let solution = solution.unwrap();

        let assumptions = solution
            .bound_variables()
            .flat_map(|(var, value)| [var.geq(value), var.leq(value)])
            .collect::<Vec<_>>();

        let reasoner = LpRelaxIncr::new(encoder, unlimited_pivots());
        let mut solver = aries_solver::solver::Solver::with_extra_reasoners(model, vec![Box::new(reasoner)]);

        assert!(
            solver
                .solve_with_assumptions(&assumptions, aries_solver::solver::SearchLimit::None)
                .is_ok_and(|result| result.is_ok()),
        );
    }
}
