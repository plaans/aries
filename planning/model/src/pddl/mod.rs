pub mod convert;
pub mod input;
pub mod parser;
pub mod sexpr;

pub use parser::*;
pub use sexpr::{SAtom, SExpr, SList};

#[cfg(test)]
mod test {

    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn test_parsing_simple() -> anyhow::Result<()> {
        let domain_file = PathBuf::from("/home/abitmonnot/work/aries/planning/problems/pddl/tests/gripper.dom.pddl");
        let problem_file = PathBuf::from("/home/abitmonnot/work/aries/planning/problems/pddl/tests/gripper.pb.pddl");
        test_parsing(&domain_file, &problem_file)
    }

    fn test_parsing(domain_file: &Path, problem_file: &Path) -> anyhow::Result<()> {
        let domain_file = input::Input::from_file(domain_file)?;

        let problem_file = input::Input::from_file(problem_file)?;
        let domain = parser::parse_pddl_domain(domain_file)?;
        let problem = parser::parse_pddl_problem(problem_file)?;

        let _model = convert::build_model(&domain, &problem)?;

        Ok(())
    }
}
