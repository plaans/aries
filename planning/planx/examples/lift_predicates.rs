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
        assert!(fluent.parameters.len() == expected_num_params);
        assert!(matches!(
            &fluent.return_type, planx::Type::User(user_type)
            if user_type.members() == [expected_return_type_name]
        ));
    }

    Ok(())
}

fn main() -> Res<()> {
    simple_test(
        &PathBuf::from("planning/problems/pddl/tests/gripper.dom.pddl"),
        &PathBuf::from("planning/problems/pddl/tests/gripper.pb.pddl"),
        2,
        0,
        &[("at-robby", 0, "top-type"), ("carry:at", 1, "top-type")],
    )?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test1() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/problem.pddl"),
            4,
            0,
            &[],
        )
    }

    #[test]
    fn test2() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2004-psr-small-strips/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2004-psr-small-strips/problem.pddl"),
            5,
            5,
            &[],
        )
    }
}
