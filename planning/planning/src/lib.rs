pub mod chronicles;
pub mod classical;
pub mod parsing;
use env_param::EnvParam;

static PRINT_PLANNER_OUTPUT: EnvParam<bool> = EnvParam::new("ARIES_PRINT_PLANNER_OUTPUT", "true");
