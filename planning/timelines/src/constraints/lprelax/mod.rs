pub static LPRELAX_USE: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_USE", "none");
pub static LPRELAX_LOGS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_LOGS", "false");
/// The number of conflicts of the search after which the LP relaxation is built and posted (0: right before the first decision).
pub static LPRELAX_BUILD_AFTER_CONFLICTS: EnvParam<u64> = EnvParam::new("ARIES_LPRELAX_BUILD_AFTER_CONFLICTS", "300");
pub static LPRELAX_MERGE_EQUAL_COLUMNS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_MERGE_EQUAL_COLUMNS", "true");
pub static LPRELAX_WITH_INITIAL_STATE_SUPPORTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS", "true");
pub static LPRELAX_WITH_CONDITION_OUT_TRANSITIONS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS", "false");
/// The flow constraints of the LP relaxation: "none", "valgraph" (value graph) or "supgraph" (support graph).
pub static LPRELAX_WITH_FLOW_CONSTRS: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_WITH_FLOW_CONSTRS", "none");
pub static LPRELAX_WITH_ACHIEVERS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_WITH_ACHIEVERS", "false");
/// The goal commodities: for each goal, its presence routed from `None` to its required value (that of one of its groundings),
/// through value changes of present ground transitions (with exact capacities).
pub static LPRELAX_WITH_GOAL_COMMODITIES: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_GOAL_COMMODITIES", "false");
/// The ground support rows and columns (the lifted supports are always there). Needed by the support graph flow.
pub static LPRELAX_WITH_GROUND_SUPPORTS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_WITH_GROUND_SUPPORTS", "false");
/// The net-change rows: on a ground state variable, the number of (present) ground transitions leaving a value is at most the number of those entering it
/// (minus 1 if it's the value of a goal at the horizon).
pub static LPRELAX_WITH_NET_CHANGE_CONSTRS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_NET_CHANGE_CONSTRS", "true");
/// Only the net-change rows of the values of goals at the horizon (those with the goal term), without the others.
/// Implied by [`LPRELAX_WITH_NET_CHANGE_CONSTRS`].
pub static LPRELAX_WITH_GOAL_NET_CHANGE_CONSTRS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_GOAL_NET_CHANGE_CONSTRS", "false");
/// The linearization of the causal-graph symmetry breaking of the main model (lifted rows, needing conds not as out-transitions).
pub static LPRELAX_WITH_SYMMETRY_BREAKING: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_SYMMETRY_BREAKING", "true");
/// Binds the ground presences of sources to the lines of their grounding tables, which removes the term rows and bindings of single-source terms.
pub static LPRELAX_WITH_TABLE_LITS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_WITH_TABLE_LITS", "true");

macro_rules! lprelax_log {
    ($($arg:tt)*) => {
        if $crate::constraints::lprelax::LPRELAX_LOGS.get() {
            tracing::info!($($arg)*);
        }
    };
}

pub(crate) mod encoder;
#[cfg(test)]
mod tests;
mod transitions;
pub(crate) mod wrappers;

use aries_env_param::EnvParam;

pub(crate) use transitions::by_source::BySource;
