mod convert;
mod find_file;
pub mod input;
pub mod parser;
pub mod sexpr;

pub use convert::{build_model, build_plan};
pub use find_file::*;
pub use parser::*;
pub use sexpr::{SAtom, SExpr, SList};

#[cfg(test)]
mod test {

    use std::path::{Path, PathBuf};

    use crate::Res;

    use super::*;

    #[test]
    fn test_parsing_simple() -> Res<()> {
        let domain_file = PathBuf::from("../problems/pddl/tests/gripper.dom.pddl");
        let problem_file = PathBuf::from("../problems/pddl/tests/gripper.pb.pddl");
        test_parsing(&domain_file, &problem_file, false)
    }

    #[test]
    fn test_parsing_simple_with_lifting() -> Res<()> {
        let domain_file = PathBuf::from("../problems/pddl/tests/gripper.dom.pddl");
        let problem_file = PathBuf::from("../problems/pddl/tests/gripper.pb.pddl");
        test_parsing(&domain_file, &problem_file, true)
    }

    fn test_parsing(domain_file: &Path, problem_file: &Path, lift: bool) -> Res<()> {
        let domain_file = input::Input::from_file(domain_file)?;

        let problem_file = input::Input::from_file(problem_file)?;
        let domain = parser::parse_pddl_domain(domain_file)?;
        let problem = parser::parse_pddl_problem(problem_file)?;

        let mut _model = convert::build_model(&domain, &problem)?;
        if lift {
            crate::lift_predicates::lift_predicates_to_state_functions(&mut _model)?;
        }

        Ok(())
    }
}
