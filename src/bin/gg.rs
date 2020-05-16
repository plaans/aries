


use aries::planning::parsing::from_pddl;

fn main() -> Result<(), String> {
    let dom = std::fs::read_to_string("problems/pddl/gripper/domain.pddl")
        .map_err(|o| format!("{}", o))?;

    let prob = std::fs::read_to_string("problems/pddl/gripper/problem.pddl")
        .map_err(|o| format!("{}", o))?;

    let spec = from_pddl(&dom, &prob)?;

    Ok(())
}