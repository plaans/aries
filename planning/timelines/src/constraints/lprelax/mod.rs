pub static ARIES_LPRELAX_USE: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_USE", "none");
pub static ARIES_LPRELAX_LOGS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_LOGS", "false");
pub static ARIES_LPRELAX_MERGE_EQUAL_COLUMNS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_MERGE_EQUAL_COLUMNS", "true");
pub static ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS", "true");
pub static ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS", "true");
/// The flow constraints of the LP relaxation: "none", "valgraph" (value graph) or "supgraph" (support graph).
pub static ARIES_LPRELAX_WITH_FLOW_CONSTRS: EnvParam<String> =
    EnvParam::new("ARIES_LPRELAX_WITH_FLOW_CONSTRS", "valgraph");
pub static ARIES_LPRELAX_WITH_ACHIEVERS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_WITH_ACHIEVERS", "true");
/// The ground support rows and columns (the lifted supports are always there). Needed by the support graph flow.
pub static ARIES_LPRELAX_WITH_GROUND_SUPPORTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_GROUND_SUPPORTS", "false");
/// The net-change rows: on a ground state variable, the number of (present) ground transitions leaving a value is at most the number of those entering it
/// (minus 1 if it's the value of a goal at the horizon).
pub static ARIES_LPRELAX_WITH_NET_CHANGE_CONSTRS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_NET_CHANGE_CONSTRS", "true");

macro_rules! lprelax_log {
    ($($arg:tt)*) => {
        if $crate::constraints::lprelax::ARIES_LPRELAX_LOGS.get() {
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
