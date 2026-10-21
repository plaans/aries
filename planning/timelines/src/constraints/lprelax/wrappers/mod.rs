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

            let reasoner = LpRelaxIncr::new(encoder, 0, unlimited_pivots());
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

            let reasoner = LpRelaxIncr::new(encoder, 0, unlimited_pivots());

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
}
