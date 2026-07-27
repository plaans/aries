use std::path::{Path, PathBuf};

use planx::{Model, errors::*, lift_predicates, pddl::*};

fn parse_pddl(domain_file: &Path, problem_file: &Path) -> Res<(Model, Model)> {
    let domain_file = input::Input::from_file(domain_file)?;

    let problem_file = input::Input::from_file(problem_file)?;
    let domain = parser::parse_pddl_domain(domain_file)?;
    let problem = parser::parse_pddl_problem(problem_file)?;

    let nonlifted_model = build_model(&domain, &problem)?;
    let lifted_model = {
        let mut res = build_model(&domain, &problem)?;
        lift_predicates::lift_predicates_to_state_functions(&mut res)?;
        res
    };
    Ok((nonlifted_model, lifted_model))
}

fn get_fluent_by_name<'a>(model: &'a Model, fluent_name: &'a str) -> Res<&'a planx::Fluent> {
    Ok(model.env.fluents.get(
        model
            .env
            .fluents
            .get_by_name(fluent_name)
            .ok_or(Message::error("unknown fluent name"))?,
    ))
}

fn simple_test(
    domain_file: &Path,
    problem_file: &Path,
    expected_lifted_fluents: usize,
    expected_lifted_fluents_with_helper_types: usize,
    expected_lifted_fluents_shapes: &[(&str, usize, &str)],
) -> Res<()> {
    let (nonlifted_model, lifted_model) = parse_pddl(domain_file, problem_file)?;

    println!("== BEFORE LIFTING PREDICATES ==");
    println!("{nonlifted_model}");
    println!("== AFTER LIFTING PREDICATES ==");
    print!("{lifted_model}");

    assert!(
        nonlifted_model
            .env
            .fluents
            .iter()
            .filter(|fluent| matches!(&fluent.return_type, planx::Type::User(_)))
            .count()
            == 0
    );

    assert!(
        lifted_model
            .env
            .fluents
            .iter()
            .filter(|fluent| matches!(&fluent.return_type, planx::Type::User(_)))
            .count()
            == expected_lifted_fluents
    );
    assert!(
        lifted_model
            .env
            .fluents
            .iter()
            .filter(|fluent| matches!(
                &fluent.return_type, planx::Type::User(tpe)
                if tpe.to_single_type().unwrap().name.as_str().starts_with("_help-tpe-")
            ))
            .count()
            == expected_lifted_fluents_with_helper_types
    );

    for &(fluent_name, expected_num_params, expected_return_type_name) in expected_lifted_fluents_shapes {
        let fluent = get_fluent_by_name(&lifted_model, fluent_name)?;
        assert!(fluent.parameters.len() == expected_num_params, "{fluent_name:?}");
        assert!(
            matches!(
                &fluent.return_type, planx::Type::User(user_type)
                if user_type.members() == [expected_return_type_name]
            ),
            "{fluent_name:?}",
        );
    }

    Ok(())
}

fn main() -> Res<()> {
    simple_test(
        &PathBuf::from("planning/problems/pddl/tests/gripper.dom.pddl"),
        &PathBuf::from("planning/problems/pddl/tests/gripper.pb.pddl"),
        2,
        0,
        &[("at-robby", 0, "object"), ("carry:at", 1, "object")],
    )?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_gripper() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/pddl/tests/gripper.dom.pddl"),
            &PathBuf::from("../problems/pddl/tests/gripper.pb.pddl"),
            2,
            0,
            &[("at-robby", 0, "object"), ("carry:at", 1, "object")],
        )
    }

    #[test]
    fn test_satellite_strips() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/problem.pddl"),
            4,
            0,
            &[
                ("calibration_target", 1, "direction"),
                ("pointing", 1, "direction"),
                ("supports", 1, "mode"),
                ("on_board", 1, "satellite"),
            ],
        )
    }

    #[test]
    fn test_satellite_time() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-satellite-time-simple-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-satellite-time-simple-automatic/problem.pddl"),
            4,
            0,
            &[
                ("calibration_target", 1, "direction"),
                ("pointing", 1, "direction"),
                ("supports", 1, "mode"),
                ("on_board", 1, "satellite"),
            ],
        )
    }

    #[test]
    fn test_psr() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2004-psr-small-strips/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2004-psr-small-strips/problem.pddl"),
            5,
            5,
            &[
                (
                    "do_normal:do_wait_cb1_condeffs:do_close_sd1_condeffs",
                    0,
                    "_help-tpe-do_normal:do_wait_cb1_condeffs:do_close_sd1_condeffs",
                ),
                (
                    "not_updated_cb1:updated_cb1",
                    0,
                    "_help-tpe-not_updated_cb1:updated_cb1",
                ),
                ("closed_sd1:not_closed_sd1", 0, "_help-tpe-closed_sd1:not_closed_sd1"),
                ("closed_sd2:not_closed_sd2", 0, "_help-tpe-closed_sd2:not_closed_sd2"),
                ("closed_cb1:not_closed_cb1", 0, "_help-tpe-closed_cb1:not_closed_cb1"),
            ],
        )
    }

    #[test]
    fn test_rovers_strips() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-rovers-strips-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-rovers-strips-automatic/problem.pddl"),
            13,
            1,
            &[
                ("full:empty", 1, "_help-tpe-full:empty"),
                ("channel_free", 0, "lander"),
                ("on_board", 1, "rover"),
                ("calibration_target", 1, "objective"),
                ("store_of", 1, "rover"),
                ("available", 0, "rover"),
                ("supports", 1, "camera"),
                ("equipped_for_imaging", 0, "rover"),
                ("equipped_for_rock_analysis", 0, "rover"),
                ("equipped_for_soil_analysis", 0, "rover"),
                ("can_traverse", 2, "rover"),
                ("at_lander", 1, "waypoint"),
                ("at_", 1, "waypoint"),
            ],
        )
    }

    #[test]
    fn test_rovers_time() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-rovers-time-simple-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-rovers-time-simple-automatic/problem.pddl"),
            13,
            1,
            &[
                ("full:empty", 1, "_help-tpe-full:empty"),
                ("channel_free", 0, "lander"),
                ("on_board", 1, "rover"),
                ("calibration_target", 1, "objective"),
                ("store_of", 1, "rover"),
                ("available", 0, "rover"),
                ("supports", 1, "camera"),
                ("equipped_for_imaging", 0, "rover"),
                ("equipped_for_rock_analysis", 0, "rover"),
                ("equipped_for_soil_analysis", 0, "rover"),
                ("can_traverse", 2, "rover"),
                ("at_lander", 1, "waypoint"),
                ("at_", 1, "waypoint"),
            ],
        )
    }

    #[test]
    fn test_rovers_numeric() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-rovers-numeric-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-rovers-numeric-automatic/problem.pddl"),
            14,
            1,
            &[
                ("full:empty", 1, "_help-tpe-full:empty"),
                ("in_sun", 0, "waypoint"),
                ("channel_free", 0, "lander"),
                ("on_board", 1, "rover"),
                ("calibration_target", 1, "objective"),
                ("store_of", 1, "rover"),
                ("available", 0, "rover"),
                ("supports", 1, "camera"),
                ("equipped_for_imaging", 0, "rover"),
                ("equipped_for_rock_analysis", 0, "rover"),
                ("equipped_for_soil_analysis", 0, "rover"),
                ("can_traverse", 2, "rover"),
                ("at_lander", 1, "waypoint"),
                ("at_", 1, "waypoint"),
            ],
        )
    }
}
