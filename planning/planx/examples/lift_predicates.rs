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

fn main() -> Res<()> {
    let domain_file = PathBuf::from("planning/problems/pddl/tests/gripper.dom.pddl");
    let problem_file = PathBuf::from("planning/problems/pddl/tests/gripper.pb.pddl");

    let (nonlifted_model, lifted_model) = parse_pddl(&domain_file, &problem_file)?;

    println!("== BEFORE LIFTING PREDICATES ==");
    println!("{nonlifted_model}");
    println!("== AFTER LIFTING PREDICATES ==");
    print!("{lifted_model}");

    assert!(
        nonlifted_model
            .env
            .fluents
            .iter()
            .filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
            .count()
            == 0
    );
    assert!(
        lifted_model
            .env
            .fluents
            .iter()
            .filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
            .count()
            == 2
    );

    let atrobby_lifted = get_fluent_by_name(&lifted_model, "at-robby")?;
    assert!(atrobby_lifted.parameters.is_empty());
    assert!(matches!(atrobby_lifted.return_type, planx::Type::User(_)));

    let carry_at_lifted = get_fluent_by_name(&lifted_model, "carry:at")?;
    assert!(carry_at_lifted.parameters.len() == 1);
    assert!(matches!(carry_at_lifted.return_type, planx::Type::User(_)));

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test1() -> Res<()> {
        let domain_file = PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/domain.pddl");
        let problem_file = PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/problem.pddl");

        let (nonlifted_model, lifted_model) = parse_pddl(&domain_file, &problem_file)?;

        println!("== BEFORE LIFTING PREDICATES ==");
        println!("{nonlifted_model}");
        println!("== AFTER LIFTING PREDICATES ==");
        print!("{lifted_model}");

        assert!(
            nonlifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
                .count()
                == 0
        );
        assert!(
            lifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
                .count()
                == 4
        );

        Ok(())
    }

    #[test]
    fn test2() -> Res<()> {
        let domain_file = PathBuf::from("../problems/upf/ipc2004-psr-small-strips/domain.pddl");
        let problem_file = PathBuf::from("../problems/upf/ipc2004-psr-small-strips/problem.pddl");

        let (nonlifted_model, lifted_model) = parse_pddl(&domain_file, &problem_file)?;

        println!("== BEFORE LIFTING PREDICATES ==");
        println!("{nonlifted_model}");
        println!("== AFTER LIFTING PREDICATES ==");
        print!("{lifted_model}");

        assert!(
            nonlifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
                .count()
                == 0
        );
        assert!(
            lifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
                .count()
                == 5
        );
        assert!(
            lifted_model.env.fluents.iter().filter(|fluent| matches!(fluent.return_type, planx::Type::User(_)))
                .all(|fluent| matches!(&fluent.return_type, planx::Type::User(tpe) if tpe.to_single_type().unwrap().name.as_str().starts_with("_help-tpe-")))
        );

        Ok(())
    }
}
