pub static ARIES_LPRELAX_USE: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_USE", "none");
pub static ARIES_LPRELAX_LOGS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_LOGS", "false");
pub static ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS", "false");
pub static ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS", "true");
pub static ARIES_LPRELAX_MERGE_EQUAL_COLUMNS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_MERGE_EQUAL_COLUMNS", "true");
pub static ARIES_LPRELAX_CHECKS: EnvParam<LpRelaxChecks> = EnvParam::new("ARIES_LPRELAX_CHECKS", "once");

/// When the feasibility of the LP relaxation is checked, besides once when it is posted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LpRelaxChecks {
    /// At quiescence, only after posting the relaxation and never again
    Once,
    /// At quiscence, when reaching a new phase of fixed columns (`phases`).
    Phases,
    /// At quiescence, when some columns became fixed or unfixed since the last check (`changes`).
    Changes,
}

impl std::str::FromStr for LpRelaxChecks {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "once" => Ok(Self::Once),
            "phases" => Ok(Self::Phases),
            "changes" => Ok(Self::Changes),
            _ => Err(format!("unknown value {s:?} (expected once, phases or changes)")),
        }
    }
}

macro_rules! lprelax_log {
    ($($arg:tt)*) => {
        if $crate::constraints::lprelax::ARIES_LPRELAX_LOGS.get() {
            tracing::info!($($arg)*);
        }
    };
}

pub(crate) mod encoder;
pub(crate) mod wrappers;

pub(crate) use encoder::LpRelaxEncoder;

use aries_env_param::EnvParam;

#[cfg(test)]
mod tests {

    use super::wrappers::{LpRelaxHighs, LpRelaxIncr};
    use crate::analysis::transitions::tests::visitall::{VisitAllLine, build_and_encode_visitall_line};

    #[test]
    fn test_visitall_line_highs() {
        let sat_pb = &VisitAllLine {
            num_locs: 4,
            num_moves: 3,
        };
        let encoder = build_and_encode_visitall_line(sat_pb, false);
        let model = encoder.sched.clone().encode();

        {
            println!("Sat instance with lprelax (lprelax mustn't deem it unsat)");

            let reasoner = LpRelaxHighs::new(encoder, 0);
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

            let reasoner = LpRelaxHighs::new(encoder, 0);

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

            let reasoner = LpRelaxIncr::new(encoder, 0);
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

            let reasoner = LpRelaxIncr::new(encoder, 0);

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
