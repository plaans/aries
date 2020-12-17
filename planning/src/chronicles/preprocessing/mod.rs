mod state_variables;
mod statics;

use env_param::EnvParam;

static PREPRO_STATIC: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_STATIC", "true");
static PREPRO_STATE_VARS: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_STATE_VARS", "true");

use crate::chronicles::Problem;
pub use state_variables::predicates_as_state_variables;
pub use statics::statics_as_tables;

pub fn preprocess(problem: &mut Problem) {
    if *PREPRO_STATE_VARS.get() {
        predicates_as_state_variables(problem);
    }
    if *PREPRO_STATIC.get() {
        statics_as_tables(problem);
    }
}
