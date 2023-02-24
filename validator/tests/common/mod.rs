use anyhow::Result;
use prost::Message;
use std::fs::read;
use std::path::PathBuf;

use aries_plan_validator::interfaces::unified_planning::validate_upf;
use unified_planning::{Plan, Problem};

fn get_bin(name: &str, folder: &str) -> Result<Vec<u8>> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../planning/ext/up/bins");
    path.push(folder);
    path.push(name);
    path.set_extension("bin");
    let bin = read(path)?;
    Ok(bin)
}

fn get_problem(name: &str) -> Result<Problem> {
    let bin = get_bin(name, "problems")?;
    let problem = Problem::decode(bin.as_slice())?;
    Ok(problem)
}

fn get_plan(name: &str) -> Result<Plan> {
    let bin = get_bin(name, "plans")?;
    let plan = Plan::decode(bin.as_slice())?;
    Ok(plan)
}

pub fn valid_plan(name: &str) -> Result<()> {
    let verbose: bool = std::env::var("ARIES_DEBUG")
        .unwrap_or("false".to_owned())
        .to_lowercase()
        .parse()?;
    let problem = get_problem(name)?;
    let plan = get_plan(name)?;
    validate_upf(&problem, &plan, verbose)
}
