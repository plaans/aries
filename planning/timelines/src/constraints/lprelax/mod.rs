#[allow(dead_code)]
mod encoder;

use aries_env_param::EnvParam;

pub(crate) use encoder::LpRelaxEncoder;

pub static ARIES_LPRELAX_USE: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_USE", "false");
pub static ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_RECOVER_CLOSED_WORLD_DEFAULTS", "true");
pub static ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_WITH_CONDITION_OUT_TRANSITIONS", "true");
pub static ARIES_LPRELAX_MERGE_EQUAL_COLUMNS: EnvParam<bool> =
    EnvParam::new("ARIES_LPRELAX_MERGE_EQUAL_COLUMNS", "true");
