


use aries::planning::parsing::pddl_to_chronicles;
use aries::planning::classical::{from_chronicles, grounded_problem};
use aries::planning::classical::search::plan_search;

fn main() -> Result<(), String> {
    let dom = std::fs::read_to_string("problems/pddl/gripper/domain.pddl")
        .map_err(|o| format!("{}", o))?;

    let prob = std::fs::read_to_string("problems/pddl/gripper/problem.pddl")
        .map_err(|o| format!("{}", o))?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let lifted = from_chronicles(&spec)?;

    let grounded = grounded_problem(&lifted)?;

    let symbols = &lifted.world.table;

    match plan_search(&grounded.initial_state, &grounded.operators, &grounded.goals) {
        Some(plan) => {
            println!("Got plan: {} actions", plan.len());
            println!("=============");
            for &op in &plan {
                println!("{}",  symbols.format(grounded.operators.name(op)));
            }
        },
        None => println!("Infeasible")
    }

    Ok(())
}