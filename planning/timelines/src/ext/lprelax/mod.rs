mod encoder;
mod encoding;
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
