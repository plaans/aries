use std::path::PathBuf;

use aries_planning_model::pddl::*;

pub fn main() -> anyhow::Result<()> {
    let domain_file = PathBuf::from("/home/abitmonnot/work/aries/planning/problems/pddl/tests/gripper.dom.pddl");
    let domain_file = input::Input::from_file(&domain_file)?;

    let problem_file = PathBuf::from("/home/abitmonnot/work/aries/planning/problems/pddl/tests/gripper.pb.pddl");
    let problem_file = input::Input::from_file(&problem_file)?;
    let domain = parser::parse_pddl_domain(domain_file)?;
    let problem = parser::parse_pddl_problem(problem_file)?;

    let model = convert::build_model(&domain, &problem)?;

    Ok(())
}
