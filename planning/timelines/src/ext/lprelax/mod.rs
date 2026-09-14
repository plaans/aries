mod encoder;
mod encoding;
mod examples;
mod transitions;
pub(crate) mod wrapper;

use aries_env_param::EnvParam;

use crate::IntTerm;
use aries_solver::prelude::IntCst;

pub(crate) use encoder::LpRelaxEncoder;
use transitions::TransitionId;

use crate::ext::lprelax::transitions::TransitionGroundingId;
use crate::ext::{Source, SourceGroundingId};

pub static ARIES_LPRELAX_USE: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_USE", "false");
pub static ARIES_LPRELAX_RECOVER_MIES: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_RECOVER_MIES", "true");
pub static ARIES_LPRELAX_GROUND_2CYCLES: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_GROUND_2CYCLES", "false");
// static ARIES_LPRELAX_GROUNDER: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_GROUNDER", "simple");

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub(crate) enum ColTag {
    PresenceSource(Source),
    PresenceSourceGround(Source, SourceGroundingId),
    PresenceTransition(TransitionId),
    PresenceTransitionGround(TransitionId, TransitionGroundingId),
    Support(TransitionId, TransitionId),
    SupportGround(TransitionId, TransitionId, TransitionGroundingId, TransitionGroundingId),
    TermGround(IntTerm, IntCst),
}

#[derive(Debug, Clone)]
pub(crate) enum RowExpr {
    Eq(Vec<ColTag>, Vec<ColTag>),
    Leq(Vec<ColTag>, Vec<ColTag>),
    Geq(Vec<ColTag>, Vec<ColTag>),
    Leq1(Vec<ColTag>),
}

#[cfg(test)]
mod tests {
    use crate::ext::lprelax::LpRelaxEncoder;
    use crate::ext::lprelax::examples::visitall::{VisitAllLine, build_and_encode_visitall_line};
    use crate::ext::lprelax::wrapper::{LpRelaxReasonerWrapper, LpRelaxReasonerWrapperTrait};

    #[test]
    fn test_visitall_line() {
        let sat_pb = &VisitAllLine {
            num_locs: 4,
            num_moves: 3,
        };
        let mut encoder = build_and_encode_visitall_line(sat_pb, false);
        let model = encoder.sched.clone().encode();

        {
            println!("Sat instance with lprelax (lprelax mustn't deem it unsat)");

            let reasoner = LpRelaxReasonerWrapper::<aries_solver_lprelax::LpRelax>::new_wrapped(
                LpRelaxEncoder::new(&mut encoder),
                encoder,
            );
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
        let mut encoder = build_and_encode_visitall_line(unsat_pb, false);
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

            let reasoner = LpRelaxReasonerWrapper::<aries_solver_lprelax::LpRelax>::new_wrapped(
                LpRelaxEncoder::new(&mut encoder),
                encoder,
            );

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
