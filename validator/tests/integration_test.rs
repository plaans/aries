use anyhow::Result;
use aries_plan_validator::interfaces::unified_planning::validate_upf;

mod common;

fn valid_plan(name: &str, verbose: bool) -> Result<()> {
    let problem = common::get_problem(name)?;
    let plan = common::get_plan(name)?;
    validate_upf(&problem, &plan, verbose)
}

#[cfg(test)]
mod test {
    use super::valid_plan;

    macro_rules! make_test {
        ($name:expr) => {
            paste::item! {
                #[test]
                fn [< test_ $name >] () {
                    let result = valid_plan($name, true);
                    assert!(result.is_ok(), "\x1b[91m{:?}\x1b[0m", result.err().unwrap());
                }
            }
        };
    }

    make_test!("matchcellar");
    make_test!("basic_conditional");
    make_test!("counter_to_50");
    make_test!("robot_loader_mod");
    make_test!("hierarchical_blocks_world_exists");
    make_test!("counter");
    make_test!("travel");
    make_test!("robot_no_negative_preconditions");
    make_test!("hierarchical_blocks_world");
    make_test!("basic_without_negative_preconditions");
    make_test!("basic_forall");
    make_test!("basic_oversubscription");
    make_test!("robot_with_static_fluents_duration");
    make_test!("travel_with_consumptions");
    make_test!("robot_fluent_of_user_type");
    make_test!("hierarchical_blocks_world_with_object");
    make_test!("timed_connected_locations");
    make_test!("robot_loader");
    make_test!("basic_with_object_constant");
    make_test!("basic_nested_conjunctions");
    make_test!("complex_conditional");
    make_test!("htn-go");
    make_test!("robot_real_constants");
    make_test!("basic_exists");
    make_test!("robot_decrease");
    make_test!("basic_with_costs");
    make_test!("robot_locations_connected_without_battery");
    make_test!("robot_loader_adv");
    make_test!("basic");
    make_test!("robot_locations_connected");
    make_test!("hierarchical_blocks_world_object_as_root");
    make_test!("robot_int_battery");
    make_test!("charge_discharge");
    make_test!("robot_locations_visited");
    make_test!("temporal_conditional");
    make_test!("robot");
    make_test!("robot_fluent_of_user_type_with_int_id");
}
