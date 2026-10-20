pub static ARIES_LPRELAX_USE: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_USE", "none");
pub static ARIES_LPRELAX_LOGS: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_LOGS", "false");
pub static ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS", "false");
pub static ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS", "true");
pub static ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS", "true");
pub static ARIES_LPRELAX_MERGE_EQUAL_COLUMNS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_MERGE_EQUAL_COLUMNS", "true");

macro_rules! lprelax_log {
    ($($arg:tt)*) => {
        if $crate::constraints::lprelax::ARIES_LPRELAX_LOGS.get() {
            tracing::info!($($arg)*);
        }
    };
}

#[allow(dead_code)]
pub(crate) mod encoder;
mod tests;
#[allow(dead_code)]
mod transitions;
pub(crate) mod wrappers;

use aries_env_param::EnvParam;
