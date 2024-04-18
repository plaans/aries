mod action_rolling;
mod merge_conditions_effects;
mod mutex_predicates;
mod state_variables;
mod statics;
mod unused_effects;

use env_param::EnvParam;

static PREPRO_STATIC: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_STATIC", "true");
static PREPRO_STATE_VARS: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_STATE_VARS", "true");
static PREPRO_MUTEX_PREDICATES: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_MUTEX", "true");
static PREPRO_UNUSABLE_EFFECTS: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_UNUSABLE_EFFECTS", "true");
static PREPRO_MERGE_STATEMENTS: EnvParam<bool> = EnvParam::new("ARIES_PLANNING_PREPRO_MERGE_STATEMENTS", "true");
static PREPRO_ROLL_ACTIONS: EnvParam<bool> = EnvParam::new("ARIES_ROLL", "true");

use crate::chronicles::Problem;
pub use merge_conditions_effects::merge_conditions_effects;
pub use statics::statics_as_tables;
pub use unused_effects::merge_unusable_effects;
pub use unused_effects::remove_unusable_effects;

pub fn preprocess(problem: &mut Problem) {
    let _span = tracing::span!(tracing::Level::TRACE, "PREPRO").entered();

    if PREPRO_MUTEX_PREDICATES.get() {
        mutex_predicates::preprocess_mutex_predicates(problem);
    }

    if PREPRO_UNUSABLE_EFFECTS.get() {
        remove_unusable_effects(problem);
    }
    if PREPRO_STATE_VARS.get() {
        state_variables::lift_predicate_to_state_variables(problem);
    }
    if PREPRO_STATIC.get() {
        statics_as_tables(problem);
    }

    if PREPRO_MERGE_STATEMENTS.get() {
        merge_conditions_effects(problem);
        merge_unusable_effects(problem);
    }
    if PREPRO_ROLL_ACTIONS.get() {
        action_rolling::rollup_actions(problem)
    }
}
