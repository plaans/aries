mod state_variables;
mod statics;
mod unused_effects;

use env_param::EnvParam;

static PREPRO_STATIC: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_STATIC", "true");
static PREPRO_STATE_VARS: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_STATE_VARS", "true");
static PREPRO_UNUSABLE_EFFECTS: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_UNUSABLE_EFFECTS", "true");

use crate::chronicles::Problem;
pub use state_variables::predicates_as_state_variables;
pub use statics::statics_as_tables;
pub use unused_effects::remove_unusable_effects;

pub fn preprocess(problem: &mut Problem) {
    if PREPRO_UNUSABLE_EFFECTS.get() {
        remove_unusable_effects(problem);
    }
    if PREPRO_STATE_VARS.get() {
        predicates_as_state_variables(problem);
    }
    if PREPRO_STATIC.get() {
        statics_as_tables(problem);
    }
}
