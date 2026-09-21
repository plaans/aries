mod encoder;
pub(crate) mod wrapper;

use aries_env_param::EnvParam;

pub(crate) use encoder::LpRelaxEncoder;

pub static ARIES_LPRELAX_USE: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_USE", "false");
pub static ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS", "true");
pub static ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS", "false");

#[cfg(test)]
mod tests {
    use crate::analysis::transitions::examples::visitall::{VisitAllLine, build_and_encode_visitall_line};

    use super::wrapper::*;

    #[test]
    fn test_visitall_line() {
        let sat_pb = &VisitAllLine {
            num_locs: 4,
            num_moves: 3,
        };
        let encoder = build_and_encode_visitall_line(sat_pb, false);
        let model = encoder.sched.clone().encode();

        {
            println!("Sat instance with lprelax (lprelax mustn't deem it unsat)");

            let reasoner = LpRelaxReasonerWrapper::<aries_solver_lprelax::LpRelax>::new_wrapped(encoder, 0);
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

            let reasoner = LpRelaxReasonerWrapper::<aries_solver_lprelax::LpRelax>::new_wrapped(encoder, 0);

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
